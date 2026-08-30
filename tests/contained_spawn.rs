#[cfg(unix)]
mod unix {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::Read;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use sysprims_timeout::{
        ContainmentChild, ContainmentCompletionEvidence, TerminateTreeConfig, TreeKillReliability,
    };

    static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn serialize_process_test() -> std::sync::MutexGuard<'static, ()> {
        PROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn quick_termination() -> TerminateTreeConfig {
        TerminateTreeConfig {
            grace_timeout_ms: 50,
            kill_timeout_ms: 1_000,
            ..TerminateTreeConfig::default()
        }
    }

    fn wait_for_completion(
        guard: &mut portable_pty::ContainedPtyGuard,
    ) -> sysprims_timeout::ContainmentOutcome {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match guard.try_complete(quick_termination()) {
                Ok(Some(outcome)) => return outcome,
                Ok(None) => {}
                Err(error) => panic!("contained PTY completion failed: {:?}", error),
            }
            assert!(
                Instant::now() < deadline,
                "contained PTY child did not complete"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn natural_exit_retains_exact_child_until_guard_finalizes() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 0.1; :; exit 0"]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        assert_eq!(
            guard.tree_kill_reliability(),
            TreeKillReliability::Guaranteed
        );

        let outcome = wait_for_completion(&mut guard);
        assert!(outcome.exited);
        assert!(guard.try_complete(quick_termination()).is_err());
        assert!(guard.terminate(quick_termination()).is_err());
        assert!(guard.into_child().is_ok());
    }

    #[test]
    fn child_observes_real_controlling_terminal() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut command = CommandBuilder::new("/usr/bin/perl");
        command.args([
            "-e",
            "$| = 1; print((-t STDIN && -t STDOUT && -t STDERR) ? \"REAL_TTY\\n\" : \"NO_TTY\\n\"); sleep 60",
        ]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let mut output = [0_u8; 64];
        let count = reader.read(&mut output).unwrap();
        let output = String::from_utf8_lossy(&output[..count]);
        assert!(output.contains("REAL_TTY"), "child output: {:?}", output);
        assert!(!output.contains("NO_TTY"), "child output: {:?}", output);

        let outcome = guard.terminate(quick_termination()).unwrap();
        assert!(outcome.exited);
    }

    #[test]
    #[ignore = "run with make test-owned-pty-empty against the reviewed sysprims candidate"]
    fn owned_empty_explicit_close_with_descendant() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 60 & printf 'READY\\n'; wait"]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let mut ready = [0_u8; 64];
        let count = reader.read(&mut ready).unwrap();
        assert!(String::from_utf8_lossy(&ready[..count]).contains("READY"));
        assert_eq!(
            guard.tree_kill_reliability(),
            TreeKillReliability::Guaranteed
        );
        let outcome = guard.terminate(quick_termination()).unwrap();

        assert!(outcome.exited);
        assert_eq!(
            outcome.tree_kill_reliability,
            TreeKillReliability::Guaranteed
        );
        assert!(matches!(
            outcome.completion,
            ContainmentCompletionEvidence::Empty { .. }
        ));
        let mut child = match guard.into_child() {
            Ok(child) => child,
            Err(_) => panic!("finalized guard did not return the exact child"),
        };
        assert!(ContainmentChild::try_wait(&mut child).unwrap());
    }

    #[test]
    #[ignore = "run with make test-owned-pty-empty against the reviewed sysprims candidate"]
    fn owned_empty_natural_leader_exit_with_descendant() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 60 & sleep 60 & printf 'READY\\n'; exit 0"]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let mut ready = [0_u8; 64];
        let count = reader.read(&mut ready).unwrap();
        assert!(String::from_utf8_lossy(&ready[..count]).contains("READY"));
        assert_eq!(
            guard.tree_kill_reliability(),
            TreeKillReliability::Guaranteed
        );
        let outcome = wait_for_completion(&mut guard);

        assert!(outcome.exited);
        assert_eq!(
            outcome.tree_kill_reliability,
            TreeKillReliability::Guaranteed
        );
        assert!(matches!(
            outcome.completion,
            ContainmentCompletionEvidence::Empty { .. }
        ));
        let mut child = match guard.into_child() {
            Ok(child) => child,
            Err(_) => panic!("finalized guard did not return the exact child"),
        };
        assert!(ContainmentChild::try_wait(&mut child).unwrap());
    }

    #[test]
    fn presentation_handles_can_close_before_guard() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut command = CommandBuilder::new("/usr/bin/perl");
        command.args([
            "-e",
            "$SIG{HUP} = 'IGNORE'; $| = 1; print \"READY\\n\"; sleep 60",
        ]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let mut ready = [0_u8; 64];
        let count = reader.read(&mut ready).unwrap();
        assert!(String::from_utf8_lossy(&ready[..count]).contains("READY"));
        drop(reader);
        drop(pair);

        let outcome = guard.terminate(quick_termination()).unwrap();
        assert!(outcome.exited);
        assert!(guard.into_child().is_ok());
    }

    #[test]
    fn command_resolution_failure_happens_before_spawn() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let command = CommandBuilder::new("/definitely/not/a/real/executable");

        let error = match pair.slave.spawn_contained_command(command) {
            Ok(_) => panic!("missing command must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("doesn't exist"));
    }

    #[test]
    fn rapid_exit_acknowledgement_races_remain_owned() {
        let _serial = serialize_process_test();
        for _ in 0..32 {
            let pair = native_pty_system().openpty(PtySize::default()).unwrap();
            let command = CommandBuilder::new("/usr/bin/true");
            let mut guard = pair.slave.spawn_contained_command(command).unwrap();

            let outcome = wait_for_completion(&mut guard);
            assert!(outcome.exited);
            assert!(guard.into_child().is_ok());
        }
    }
}

#[cfg(windows)]
#[test]
fn guaranteed_conpty_containment_owns_job_and_child() {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use sysprims_timeout::{ContainmentBoundaryStrength, TerminateTreeConfig, TreeKillReliability};

    let marker = std::env::temp_dir().join(format!(
        "portable-pty-contained-spawn-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos()
    ));
    let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
    command.args(["--exact", "windows_contained_spawn_helper", "--nocapture"]);
    command.env("SYSPRIMS_PTY_TEST_MODE", "contained_spawn");
    command.env("SYSPRIMS_PTY_CONTAINED_MARKER", &marker);
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    serve_headless_windows_pty(pair.master.as_ref());
    let mut guard = pair.slave.spawn_contained_command(command).unwrap();

    assert_eq!(
        guard.tree_kill_reliability(),
        TreeKillReliability::Guaranteed
    );
    assert_eq!(
        guard.boundary_strength(),
        ContainmentBoundaryStrength::KernelEnforcedJob
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "contained child did not run");
        std::thread::sleep(Duration::from_millis(10));
    }
    let outcome = guard
        .terminate(TerminateTreeConfig::default())
        .expect("contained ConPTY termination failed");
    assert!(outcome.exited);
    assert_eq!(
        outcome.boundary_strength,
        ContainmentBoundaryStrength::KernelEnforcedJob
    );
    let _ = std::fs::remove_file(marker);
}

#[cfg(windows)]
#[test]
fn windows_contained_spawn_helper() {
    use std::time::Duration;

    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("contained_spawn") {
        return;
    }
    std::fs::write(
        std::env::var_os("SYSPRIMS_PTY_CONTAINED_MARKER").unwrap(),
        b"started",
    )
    .expect("failed to write contained-spawn marker");
    std::thread::sleep(Duration::from_secs(30));
}

#[cfg(windows)]
fn windows_fixture_path(label: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};

    std::env::temp_dir().join(format!(
        "sysprims-pty-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos()
    ))
}

#[cfg(windows)]
fn wait_for_pid_file(path: &std::path::Path) -> u32 {
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(value) = std::fs::read_to_string(path) {
            return value.trim().parse().expect("fixture PID is invalid");
        }
        assert!(Instant::now() < deadline, "fixture PID was not reported");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn open_process_for_wait(pid: u32) -> std::os::windows::io::OwnedHandle {
    use std::os::windows::io::FromRawHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::{PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZE};

    let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    assert!(!handle.is_null(), "failed to open fixture process {}", pid);
    unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle.cast()) }
}

#[cfg(windows)]
fn assert_process_exited(handle: &std::os::windows::io::OwnedHandle) {
    use std::os::windows::io::AsRawHandle;
    use winapi::um::synchapi::WaitForSingleObject;
    use winapi::um::winbase::WAIT_OBJECT_0;

    let result = unsafe { WaitForSingleObject(handle.as_raw_handle().cast(), 5_000) };
    assert_eq!(result, WAIT_OBJECT_0, "Job member survived termination");
}

#[cfg(windows)]
fn serve_headless_windows_pty(master: &dyn portable_pty::MasterPty) {
    let mut reader = master.try_clone_reader().unwrap();
    let mut writer = master.take_writer().unwrap();
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0; 1_024];
        loop {
            let Ok(read) = reader.read(&mut chunk) else {
                return;
            };
            if read == 0 {
                return;
            }
            output.extend_from_slice(&chunk[..read]);
            if output.windows(4).any(|window| window == b"\x1b[6n") {
                use std::io::Write as _;
                writer.write_all(b"\x1b[1;1R").unwrap();
                writer.flush().unwrap();
                output.clear();
            } else if output.len() > 3 {
                output.drain(..output.len() - 3);
            }
        }
    });
}

#[cfg(windows)]
#[test]
fn immediate_child_and_grandchild_remain_in_owned_job() {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use sysprims_timeout::{ContainmentCompletionEvidence, TerminateTreeConfig};

    let child_pid_file = windows_fixture_path("child-pid");
    let grandchild_pid_file = windows_fixture_path("grandchild-pid");
    let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
    command.args(["--exact", "windows_tree_parent_helper", "--nocapture"]);
    command.env("SYSPRIMS_PTY_TEST_MODE", "tree_parent");
    command.env("SYSPRIMS_PTY_CHILD_PID_FILE", &child_pid_file);
    command.env("SYSPRIMS_PTY_GRANDCHILD_PID_FILE", &grandchild_pid_file);

    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    serve_headless_windows_pty(pair.master.as_ref());
    let mut guard = pair.slave.spawn_contained_command(command).unwrap();
    let child_handle = open_process_for_wait(wait_for_pid_file(&child_pid_file));
    let grandchild_handle = open_process_for_wait(wait_for_pid_file(&grandchild_pid_file));

    let outcome = guard
        .terminate(TerminateTreeConfig::default())
        .expect("Job termination failed");
    assert!(outcome.exited);
    assert!(matches!(
        outcome.completion,
        ContainmentCompletionEvidence::Empty { .. }
    ));
    assert_process_exited(&child_handle);
    assert_process_exited(&grandchild_handle);

    let _ = std::fs::remove_file(child_pid_file);
    let _ = std::fs::remove_file(grandchild_pid_file);
}

#[cfg(windows)]
#[test]
fn create_breakaway_from_job_cannot_escape() {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::time::{Duration, Instant};
    use sysprims_timeout::TerminateTreeConfig;

    let result_file = windows_fixture_path("breakaway-result");
    let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
    command.args(["--exact", "windows_breakaway_parent_helper", "--nocapture"]);
    command.env("SYSPRIMS_PTY_TEST_MODE", "breakaway_parent");
    command.env("SYSPRIMS_PTY_BREAKAWAY_RESULT_FILE", &result_file);

    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    serve_headless_windows_pty(pair.master.as_ref());
    let mut guard = pair.slave.spawn_contained_command(command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = loop {
        if let Ok(result) = std::fs::read_to_string(&result_file) {
            break result;
        }
        assert!(
            Instant::now() < deadline,
            "breakaway result was not reported"
        );
        std::thread::sleep(Duration::from_millis(10));
    };

    let outcome = guard
        .terminate(TerminateTreeConfig::default())
        .expect("Job termination failed");
    assert!(outcome.exited);
    assert!(
        result.starts_with("denied:"),
        "CREATE_BREAKAWAY_FROM_JOB unexpectedly escaped: {}",
        result
    );
    let _ = std::fs::remove_file(result_file);
}

#[cfg(windows)]
#[test]
fn windows_tree_parent_helper() {
    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("tree_parent") {
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "windows_tree_child_helper", "--nocapture"])
        .env("SYSPRIMS_PTY_TEST_MODE", "tree_child")
        .env(
            "SYSPRIMS_PTY_CHILD_PID_FILE",
            std::env::var_os("SYSPRIMS_PTY_CHILD_PID_FILE").unwrap(),
        )
        .env(
            "SYSPRIMS_PTY_GRANDCHILD_PID_FILE",
            std::env::var_os("SYSPRIMS_PTY_GRANDCHILD_PID_FILE").unwrap(),
        )
        .spawn()
        .expect("failed to spawn immediate child helper");
    child.wait().expect("immediate child wait failed");
}

#[cfg(windows)]
#[test]
fn windows_tree_child_helper() {
    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("tree_child") {
        return;
    }
    std::fs::write(
        std::env::var_os("SYSPRIMS_PTY_CHILD_PID_FILE").unwrap(),
        std::process::id().to_string(),
    )
    .expect("failed to write immediate child PID");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "windows_tree_grandchild_helper", "--nocapture"])
        .env("SYSPRIMS_PTY_TEST_MODE", "tree_grandchild")
        .env(
            "SYSPRIMS_PTY_GRANDCHILD_PID_FILE",
            std::env::var_os("SYSPRIMS_PTY_GRANDCHILD_PID_FILE").unwrap(),
        )
        .spawn()
        .expect("failed to spawn grandchild helper");
    child.wait().expect("grandchild wait failed");
}

#[cfg(windows)]
#[test]
fn windows_tree_grandchild_helper() {
    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("tree_grandchild") {
        return;
    }
    std::fs::write(
        std::env::var_os("SYSPRIMS_PTY_GRANDCHILD_PID_FILE").unwrap(),
        std::process::id().to_string(),
    )
    .expect("failed to write grandchild PID");
    std::thread::sleep(std::time::Duration::from_secs(30));
}

#[cfg(windows)]
#[test]
fn windows_breakaway_parent_helper() {
    use std::os::windows::process::CommandExt;
    use winapi::um::winbase::CREATE_BREAKAWAY_FROM_JOB;

    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("breakaway_parent") {
        return;
    }
    let result_file = std::env::var_os("SYSPRIMS_PTY_BREAKAWAY_RESULT_FILE").unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "windows_breakaway_leaf_helper", "--nocapture"])
        .env("SYSPRIMS_PTY_TEST_MODE", "breakaway_leaf")
        .creation_flags(CREATE_BREAKAWAY_FROM_JOB)
        .spawn();
    match result {
        Err(error) => {
            std::fs::write(
                result_file,
                format!("denied:{}", error.raw_os_error().unwrap_or(0)),
            )
            .expect("failed to write breakaway denial");
        }
        Ok(mut escaped) => {
            let pid = escaped.id();
            let _ = escaped.kill();
            let _ = escaped.wait();
            std::fs::write(result_file, format!("escaped:{pid}"))
                .expect("failed to write breakaway escape");
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_breakaway_leaf_helper() {
    if std::env::var("SYSPRIMS_PTY_TEST_MODE").as_deref() != Ok("breakaway_leaf") {
        return;
    }
    std::thread::sleep(std::time::Duration::from_secs(30));
}
