#![cfg(unix)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysprims_timeout::TerminateTreeConfig;

struct ForkSensitiveAllocator;

static ALLOCATOR_LOCK: Mutex<()> = Mutex::new(());
static PARENT_PID: AtomicI32 = AtomicI32::new(0);
static HOLD_LOCK: AtomicBool = AtomicBool::new(true);
static LOCK_HELD: AtomicBool = AtomicBool::new(false);

impl ForkSensitiveAllocator {
    fn block_if_fork_child() {
        let parent_pid = PARENT_PID.load(Ordering::Acquire);
        if parent_pid > 0 && unsafe { libc::getpid() } != parent_pid {
            let _inherited_lock = ALLOCATOR_LOCK.lock().unwrap();
        }
    }
}

unsafe impl GlobalAlloc for ForkSensitiveAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::block_if_fork_child();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::block_if_fork_child();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::block_if_fork_child();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::block_if_fork_child();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: ForkSensitiveAllocator = ForkSensitiveAllocator;

#[test]
fn contained_pre_exec_does_not_touch_inherited_allocator_lock() {
    PARENT_PID.store(unsafe { libc::getpid() }, Ordering::Release);

    let lock_holder = std::thread::spawn(|| {
        let _guard = ALLOCATOR_LOCK.lock().unwrap();
        LOCK_HELD.store(true, Ordering::Release);
        while HOLD_LOCK.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    });

    while !LOCK_HELD.load(Ordering::Acquire) {
        std::thread::yield_now();
    }

    let completed = std::sync::Arc::new(AtomicBool::new(false));
    let watchdog_completed = std::sync::Arc::clone(&completed);
    let watchdog = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !watchdog_completed.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !watchdog_completed.load(Ordering::Acquire) {
            unsafe { libc::_exit(124) };
        }
    });

    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    let mut command = CommandBuilder::new("/usr/bin/true");
    command.set_controlling_tty(false);
    let mut guard = pair.slave.spawn_contained_command(command).unwrap();

    HOLD_LOCK.store(false, Ordering::Release);
    lock_holder.join().unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if guard
            .try_complete(TerminateTreeConfig {
                grace_timeout_ms: 50,
                kill_timeout_ms: 1_000,
                ..TerminateTreeConfig::default()
            })
            .unwrap()
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }

    completed.store(true, Ordering::Release);
    watchdog.join().unwrap();
}
