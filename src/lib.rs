//! This crate provides a cross platform API for working with the
//! psuedo terminal (pty) interfaces provided by the system.
//! Unlike other crates in this space, this crate provides a set
//! of traits that allow selecting from different implementations
//! at runtime.
//! This crate is part of [wezterm](https://github.com/wezterm/wezterm).
//!
//! ```no_run
//! use portable_pty::{CommandBuilder, PtySize, native_pty_system, PtySystem};
//! use anyhow::Error;
//!
//! // Use the native pty implementation for the system
//! let pty_system = native_pty_system();
//!
//! // Create a new pty
//! let mut pair = pty_system.openpty(PtySize {
//!     rows: 24,
//!     cols: 80,
//!     // Not all systems support pixel_width, pixel_height,
//!     // but it is good practice to set it to something
//!     // that matches the size of the selected font.  That
//!     // is more complex than can be shown here in this
//!     // brief example though!
//!     pixel_width: 0,
//!     pixel_height: 0,
//! })?;
//!
//! // Spawn a shell into the pty
//! let cmd = CommandBuilder::new("bash");
//! let child = pair.slave.spawn_command(cmd)?;
//!
//! // Read and parse output from the pty with reader
//! let mut reader = pair.master.try_clone_reader()?;
//!
//! // Send data to the pty by writing to the master
//! writeln!(pair.master.take_writer()?, "ls -l\r\n")?;
//! # Ok::<(), Error>(())
//! ```
//!
use anyhow::Error;
use downcast_rs::{impl_downcast, Downcast};
#[cfg(feature = "serde_support")]
use serde_derive::*;
use std::io::Result as IoResult;
#[cfg(windows)]
use std::os::windows::prelude::{AsRawHandle, RawHandle};

/// A PTY child whose exclusive reap ownership can be transferred to sysprims.
///
/// Callers receive this type only after a containment guard has finalized.
/// While containment is active, the guard keeps the child handle private.
#[derive(Debug)]
pub struct ContainedPtyChild {
    pub(crate) child: std::process::Child,
}

impl ContainedPtyChild {
    #[cfg(unix)]
    pub(crate) fn new(child: std::process::Child) -> Self {
        Self { child }
    }
}

/// Stage at which a guarded PTY spawn failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainedPtySpawnErrorStage {
    /// No child was created.
    BeforeSpawn,
    /// The same-spawn receipt could not be validated.
    Receipt,
    /// The exact child and receipt could not be adopted into a guard.
    Adoption,
}

#[cfg(unix)]
enum FailedChildRecoveryCommand {
    Adopt(ContainedPtyChild),
    Attempt {
        timeout: std::time::Duration,
        #[cfg(test)]
        fault: FailedChildRecoveryFault,
        response: std::sync::mpsc::SyncSender<IoResult<bool>>,
    },
}

#[cfg(all(test, unix))]
#[derive(Clone, Copy)]
enum FailedChildRecoveryFault {
    None,
    KillError,
    Deadline,
}

#[cfg(unix)]
struct FailedChildRecovery {
    sender: std::sync::mpsc::Sender<FailedChildRecoveryCommand>,
    pending: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(unix)]
impl FailedChildRecovery {
    fn prepare() -> IoResult<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let pending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_pending = std::sync::Arc::clone(&pending);
        std::thread::Builder::new()
            .name("sysprims-pty-recovery".to_string())
            .spawn(move || failed_child_recovery_worker(receiver, worker_pending))?;
        Ok(Self { sender, pending })
    }

    fn attach(&self, child: ContainedPtyChild) {
        self.pending
            .store(true, std::sync::atomic::Ordering::Release);
        if let Err(error) = self.sender.send(FailedChildRecoveryCommand::Adopt(child)) {
            let child = match error.0 {
                FailedChildRecoveryCommand::Adopt(child) => child,
                FailedChildRecoveryCommand::Attempt { .. } => unreachable!(),
            };
            bounded_local_recovery_or_abort(child);
        }
    }

    fn attempt(&self, timeout: std::time::Duration) -> IoResult<bool> {
        self.attempt_inner(
            timeout,
            #[cfg(test)]
            FailedChildRecoveryFault::None,
        )
    }

    fn attempt_inner(
        &self,
        timeout: std::time::Duration,
        #[cfg(test)] fault: FailedChildRecoveryFault,
    ) -> IoResult<bool> {
        if !self.pending.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(true);
        }

        let timeout = timeout.min(std::time::Duration::from_secs(2));
        let (response, result) = std::sync::mpsc::sync_channel(1);
        self.sender
            .send(FailedChildRecoveryCommand::Attempt {
                timeout,
                #[cfg(test)]
                fault,
                response,
            })
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "exact-child recovery owner stopped",
                )
            })?;
        result
            .recv_timeout(timeout + std::time::Duration::from_millis(250))
            .map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("exact-child recovery response unavailable: {error}"),
                )
            })?
    }

    #[cfg(test)]
    fn attempt_with_fault(
        &self,
        timeout: std::time::Duration,
        fault: FailedChildRecoveryFault,
    ) -> IoResult<bool> {
        self.attempt_inner(timeout, fault)
    }

    fn is_pending(&self) -> bool {
        self.pending.load(std::sync::atomic::Ordering::Acquire)
    }
}

#[cfg(unix)]
fn failed_child_recovery_worker(
    receiver: std::sync::mpsc::Receiver<FailedChildRecoveryCommand>,
    pending: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut child = None;
    while let Ok(command) = receiver.recv() {
        match command {
            FailedChildRecoveryCommand::Adopt(adopted) => {
                debug_assert!(child.is_none());
                child = Some(adopted);
            }
            FailedChildRecoveryCommand::Attempt {
                timeout,
                #[cfg(test)]
                fault,
                response,
            } => {
                let result = attempt_failed_child_recovery(
                    &mut child,
                    timeout,
                    #[cfg(test)]
                    fault,
                );
                pending.store(child.is_some(), std::sync::atomic::Ordering::Release);
                let _ = response.send(result);
            }
        }
    }

    // Disconnect means the public error/controller was dropped. Ownership
    // stays on this parent-side worker; the destructor itself never blocks.
    while child.is_some() {
        let _ = attempt_failed_child_recovery(
            &mut child,
            std::time::Duration::from_secs(2),
            #[cfg(test)]
            FailedChildRecoveryFault::None,
        );
        pending.store(child.is_some(), std::sync::atomic::Ordering::Release);
        if child.is_some() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(unix)]
fn attempt_failed_child_recovery(
    child: &mut Option<ContainedPtyChild>,
    timeout: std::time::Duration,
    #[cfg(test)] fault: FailedChildRecoveryFault,
) -> IoResult<bool> {
    let owned_child = match child.as_mut() {
        Some(child) => child,
        None => return Ok(true),
    };

    if owned_child.child.try_wait()?.is_some() {
        child.take();
        return Ok(true);
    }

    #[cfg(test)]
    match fault {
        FailedChildRecoveryFault::KillError => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected exact-child kill failure",
            ));
        }
        FailedChildRecoveryFault::Deadline => {
            std::thread::sleep(timeout.min(std::time::Duration::from_millis(10)));
            return Ok(false);
        }
        FailedChildRecoveryFault::None => {}
    }

    // This is the exact spawn-owned std::process::Child handle. Never
    // reconstruct signal authority from an observed PID.
    owned_child.child.kill()?;

    let timeout = timeout.min(std::time::Duration::from_secs(2));
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match owned_child.child.try_wait()? {
            Some(_) => {
                child.take();
                return Ok(true);
            }
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            None => return Ok(false),
        }
    }
}

#[cfg(unix)]
fn bounded_local_recovery_or_abort(mut child: ContainedPtyChild) -> ! {
    let _ = child.child.kill();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match child.child.try_wait() {
            Ok(Some(_)) => {
                std::process::abort();
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(None) | Err(_) => std::process::abort(),
        }
    }
}

/// Error from a guarded PTY spawn.
///
/// Errors after child creation retain an opaque exact-child recovery state.
/// The active child handle is never returned to the caller or discarded.
/// Dropping an error disconnects its bounded controller; a parent-side worker
/// keeps exact-child ownership through direct kill and reap.
pub struct ContainedPtySpawnError {
    stage: ContainedPtySpawnErrorStage,
    source: Error,
    #[cfg(unix)]
    recovery: Option<FailedChildRecovery>,
}

impl ContainedPtySpawnError {
    pub(crate) fn before_spawn(source: Error) -> Self {
        Self {
            stage: ContainedPtySpawnErrorStage::BeforeSpawn,
            source,
            #[cfg(unix)]
            recovery: None,
        }
    }

    #[cfg(unix)]
    pub(crate) fn after_spawn(
        stage: ContainedPtySpawnErrorStage,
        source: Error,
        child: ContainedPtyChild,
        recovery: FailedChildRecovery,
    ) -> Self {
        debug_assert!(stage != ContainedPtySpawnErrorStage::BeforeSpawn);
        recovery.attach(child);
        let mut error = Self {
            stage,
            source,
            recovery: Some(recovery),
        };
        let _ = error.recover(std::time::Duration::from_secs(2));
        error
    }

    #[cfg(all(test, unix))]
    pub(crate) fn after_spawn_pending_for_test(
        stage: ContainedPtySpawnErrorStage,
        source: Error,
        child: ContainedPtyChild,
        recovery: FailedChildRecovery,
    ) -> Self {
        debug_assert!(stage != ContainedPtySpawnErrorStage::BeforeSpawn);
        recovery.attach(child);
        Self {
            stage,
            source,
            recovery: Some(recovery),
        }
    }

    /// Report the stage at which the spawn failed.
    pub fn stage(&self) -> ContainedPtySpawnErrorStage {
        self.stage
    }

    /// Whether exact-child recovery is still in progress.
    pub fn recovery_pending(&self) -> bool {
        #[cfg(unix)]
        {
            self.recovery
                .as_ref()
                .is_some_and(FailedChildRecovery::is_pending)
        }

        #[cfg(not(unix))]
        {
            false
        }
    }

    /// Make one bounded exact-child recovery attempt.
    ///
    /// The attempt is capped at two seconds. Returns `true` when no child
    /// remains to reap.
    pub fn recover(&mut self, timeout: std::time::Duration) -> IoResult<bool> {
        #[cfg(unix)]
        {
            match self.recovery.as_mut() {
                Some(recovery) => recovery.attempt(timeout),
                None => Ok(true),
            }
        }

        #[cfg(not(unix))]
        {
            let _ = timeout;
            Ok(true)
        }
    }
}

impl std::fmt::Debug for ContainedPtySpawnError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContainedPtySpawnError")
            .field("stage", &self.stage)
            .field("source", &self.source)
            .field("recovery_pending", &self.recovery_pending())
            .finish()
    }
}

impl std::fmt::Display for ContainedPtySpawnError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for ContainedPtySpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// An owned, spawn-time-acquired PTY containment capability.
pub type ContainedPtyGuard = sysprims_timeout::ContainmentGuard<ContainedPtyChild>;

pub mod cmdbuilder;
pub use cmdbuilder::CommandBuilder;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod win;

pub mod serial;

/// Represents the size of the visible display area in the pty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct PtySize {
    /// The number of lines of text
    pub rows: u16,
    /// The number of columns of text
    pub cols: u16,
    /// The width of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_width: u16,
    /// The height of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Represents the master/control end of the pty
pub trait MasterPty: Downcast + Send {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    /// Retrieves the size of the pty as known by the kernel
    fn get_size(&self) -> Result<PtySize, Error>;
    /// Obtain a readable handle; output from the slave(s) is readable
    /// via this stream.
    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error>;
    /// Obtain a writable handle; writing to it will send data to the
    /// slave end.
    /// Dropping the writer will send EOF to the slave end.
    /// It is invalid to take the writer more than once.
    fn take_writer(&self) -> Result<Box<dyn std::io::Write + Send>, Error>;

    /// If applicable to the type of the tty, return the local process id
    /// of the process group or session leader
    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t>;

    /// If get_termios() and process_group_leader() are both implemented and
    /// return Some, then as_raw_fd() should return the same underlying fd
    /// associated with the stream. This is to enable applications that
    /// "know things" to query similar information for themselves.
    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<unix::RawFd>;

    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf>;

    /// If applicable to the type of the tty, return the termios
    /// associated with the stream
    #[cfg(unix)]
    fn get_termios(&self) -> Option<nix::sys::termios::Termios> {
        None
    }
}
impl_downcast!(MasterPty);

/// Represents a child process spawned into the pty.
/// This handle can be used to wait for or terminate that child process.
pub trait Child: std::fmt::Debug + ChildKiller + Downcast + Send {
    /// Poll the child to see if it has completed.
    /// Does not block.
    /// Returns None if the child has not yet terminated,
    /// else returns its exit status.
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>>;
    /// Blocks execution until the child process has completed,
    /// yielding its exit status.
    fn wait(&mut self) -> IoResult<ExitStatus>;
    /// Returns the process identifier of the child process,
    /// if applicable
    fn process_id(&self) -> Option<u32>;
    /// Returns the process handle of the child process, if applicable.
    /// Only available on Windows.
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle>;
}
impl_downcast!(Child);

/// Represents the ability to signal a Child to terminate
pub trait ChildKiller: std::fmt::Debug + Downcast + Send {
    /// Terminate the child process
    fn kill(&mut self) -> IoResult<()>;

    /// Clone an object that can be split out from the Child in order
    /// to send it signals independently from a thread that may be
    /// blocked in `.wait`.
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync>;
}
impl_downcast!(ChildKiller);

/// Represents the slave side of a pty.
/// Can be used to spawn processes into the pty.
pub trait SlavePty {
    /// Spawns the command specified by the provided CommandBuilder
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<dyn Child + Send + Sync>, Error>;

    /// Spawns a command with a sealed same-spawn sysprims containment guard.
    ///
    /// Native Unix PTYs provide guaranteed spawn-time acquisition. Other PTY
    /// implementations reject this operation before spawning a process.
    fn spawn_contained_command(
        &self,
        cmd: CommandBuilder,
    ) -> Result<ContainedPtyGuard, ContainedPtySpawnError> {
        let _ = cmd;
        Err(ContainedPtySpawnError::before_spawn(anyhow::anyhow!(
            "spawn-time PTY containment is unavailable for this PTY implementation"
        )))
    }
}

/// Represents the exit status of a child process.
#[derive(Debug, Clone)]
pub struct ExitStatus {
    code: u32,
    signal: Option<String>,
}

impl ExitStatus {
    /// Construct an ExitStatus from a process return code
    pub fn with_exit_code(code: u32) -> Self {
        Self { code, signal: None }
    }

    /// Construct an ExitStatus from a signal name
    pub fn with_signal(signal: &str) -> Self {
        Self {
            code: 1,
            signal: Some(signal.to_string()),
        }
    }

    /// Returns true if the status indicates successful completion
    pub fn success(&self) -> bool {
        match self.signal {
            None => self.code == 0,
            Some(_) => false,
        }
    }

    /// Returns the exit code that this ExitStatus was constructed with
    pub fn exit_code(&self) -> u32 {
        self.code
    }

    /// Returns the signal if present that this ExitStatus was constructed with
    pub fn signal(&self) -> Option<&str> {
        self.signal.as_deref()
    }
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(status: std::process::ExitStatus) -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;

            if let Some(signal) = status.signal() {
                let signame = unsafe { libc::strsignal(signal) };
                let signal = if signame.is_null() {
                    format!("Signal {}", signal)
                } else {
                    let signame = unsafe { std::ffi::CStr::from_ptr(signame) };
                    signame.to_string_lossy().to_string()
                };

                return ExitStatus {
                    code: status.code().map(|c| c as u32).unwrap_or(1),
                    signal: Some(signal),
                };
            }
        }

        let code =
            status
                .code()
                .map(|c| c as u32)
                .unwrap_or_else(|| if status.success() { 0 } else { 1 });

        ExitStatus { code, signal: None }
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.success() {
            write!(fmt, "Success")
        } else {
            match &self.signal {
                Some(sig) => write!(fmt, "Terminated by {}", sig),
                None => write!(fmt, "Exited with code {}", self.code),
            }
        }
    }
}

pub struct PtyPair {
    // slave is listed first so that it is dropped first.
    // The drop order is stable and specified by rust rfc 1857
    pub slave: Box<dyn SlavePty + Send>,
    pub master: Box<dyn MasterPty + Send>,
}

/// The `PtySystem` trait allows an application to work with multiple
/// possible Pty implementations at runtime.  This is important on
/// Windows systems which have a variety of implementations.
pub trait PtySystem: Downcast {
    /// Create a new Pty instance with the window size set to the specified
    /// dimensions.  Returns a (master, slave) Pty pair.  The master side
    /// is used to drive the slave side.
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair>;
}
impl_downcast!(PtySystem);

impl Child for std::process::Child {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        std::process::Child::try_wait(self).map(|status| status.map(Into::into))
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        std::process::Child::wait(self).map(Into::into)
    }

    fn process_id(&self) -> Option<u32> {
        Some(self.id())
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        Some(std::os::windows::io::AsRawHandle::as_raw_handle(self))
    }
}

impl Child for ContainedPtyChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        std::process::Child::try_wait(&mut self.child).map(|status| status.map(Into::into))
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        std::process::Child::wait(&mut self.child).map(Into::into)
    }

    fn process_id(&self) -> Option<u32> {
        Some(self.child.id())
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        Some(std::os::windows::io::AsRawHandle::as_raw_handle(
            &self.child,
        ))
    }
}

impl sysprims_timeout::ContainmentChild for ContainedPtyChild {
    fn process_id(&self) -> Option<u32> {
        Some(self.child.id())
    }

    fn try_wait(&mut self) -> IoResult<bool> {
        std::process::Child::try_wait(&mut self.child).map(|status| status.is_some())
    }

    #[cfg(windows)]
    fn raw_process_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        Some(std::os::windows::io::AsRawHandle::as_raw_handle(
            &self.child,
        ))
    }
}

#[derive(Debug)]
struct ProcessSignaller {
    pid: Option<u32>,

    #[cfg(windows)]
    handle: Option<filedescriptor::OwnedHandle>,
}

#[cfg(windows)]
impl ChildKiller for ProcessSignaller {
    fn kill(&mut self) -> IoResult<()> {
        if let Some(handle) = &self.handle {
            unsafe {
                if winapi::um::processthreadsapi::TerminateProcess(handle.as_raw_handle() as _, 127)
                    == 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }
        Ok(())
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self {
            pid: self.pid,
            handle: self.handle.as_ref().and_then(|h| h.try_clone().ok()),
        })
    }
}

#[cfg(unix)]
impl ChildKiller for ProcessSignaller {
    fn kill(&mut self) -> IoResult<()> {
        if let Some(pid) = self.pid {
            let result = unsafe { libc::kill(pid as i32, libc::SIGHUP) };
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self { pid: self.pid })
    }
}

impl ChildKiller for std::process::Child {
    fn kill(&mut self) -> IoResult<()> {
        #[cfg(unix)]
        {
            // On unix, we send the SIGHUP signal instead of trying to kill
            // the process. The default behavior of a process receiving this
            // signal is to be killed unless it configured a signal handler.
            let result = unsafe { libc::kill(self.id() as i32, libc::SIGHUP) };
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            // We successfully delivered SIGHUP, but the semantics of Child::kill
            // are that on success the process is dead or shortly about to
            // terminate.  Since SIGUP doesn't guarantee termination, we
            // give the process a bit of a grace period to shutdown or do whatever
            // it is doing in its signal handler befre we proceed with the
            // full on kill.
            for attempt in 0..5 {
                if attempt > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                if let Ok(Some(_)) = self.try_wait() {
                    // It completed, so report success!
                    return Ok(());
                }
            }

            // it's still alive after a grace period, so proceed with a kill
        }

        std::process::Child::kill(self)
    }

    #[cfg(windows)]
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        struct RawDup(RawHandle);
        impl AsRawHandle for RawDup {
            fn as_raw_handle(&self) -> RawHandle {
                self.0
            }
        }

        Box::new(ProcessSignaller {
            pid: self.process_id(),
            handle: Child::as_raw_handle(self)
                .as_ref()
                .and_then(|h| filedescriptor::OwnedHandle::dup(&RawDup(*h)).ok()),
        })
    }

    #[cfg(unix)]
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(ProcessSignaller {
            pid: self.process_id(),
        })
    }
}

impl ChildKiller for ContainedPtyChild {
    fn kill(&mut self) -> IoResult<()> {
        ChildKiller::kill(&mut self.child)
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        ChildKiller::clone_killer(&self.child)
    }
}

pub fn native_pty_system() -> Box<dyn PtySystem + Send> {
    Box::new(NativePtySystem::default())
}

#[cfg(unix)]
pub type NativePtySystem = unix::UnixPtySystem;
#[cfg(windows)]
pub type NativePtySystem = win::conpty::ConPtySystem;
