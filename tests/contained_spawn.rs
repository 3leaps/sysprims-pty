#[cfg(unix)]
mod unix {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::Read;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use sysprims_timeout::{TerminateTreeConfig, TreeKillReliability};

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
    fn explicit_termination_escalates_for_stubborn_child() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut command = CommandBuilder::new("/usr/bin/perl");
        command.args([
            "-e",
            "$SIG{TERM} = 'IGNORE'; $| = 1; print \"READY\\n\"; sleep 60",
        ]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let mut ready = [0_u8; 64];
        let count = reader.read(&mut ready).unwrap();
        assert!(String::from_utf8_lossy(&ready[..count]).contains("READY"));
        let outcome = guard.terminate(quick_termination()).unwrap();

        assert!(outcome.exited);
        assert!(outcome.escalated);
        assert!(guard.into_child().is_ok());
    }

    #[test]
    fn natural_leader_exit_cleans_remaining_descendant() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 60 & sleep 60 & exit 0"]);

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
        let outcome = wait_for_completion(&mut guard);

        assert!(outcome.exited);
        assert!(guard.into_child().is_ok());
    }

    #[test]
    fn presentation_handles_can_close_before_guard() {
        let _serial = serialize_process_test();
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let command = CommandBuilder::new("/bin/sleep");
        let mut command = command;
        command.arg("60");

        let mut guard = pair.slave.spawn_contained_command(command).unwrap();
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
fn guaranteed_containment_rejects_before_spawn() {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    let marker = std::env::temp_dir().join(format!(
        "portable-pty-contained-spawn-{}",
        std::process::id()
    ));
    let mut command = CommandBuilder::new("cmd.exe");
    command.args(["/C", &format!("echo spawned>\"{}\"", marker.display())]);
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();

    let error = match pair.slave.spawn_contained_command(command) {
        Ok(_) => panic!("Windows guaranteed containment must reject"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("unavailable"));
    assert!(
        !marker.exists(),
        "unsupported contained spawn executed the command"
    );
}
