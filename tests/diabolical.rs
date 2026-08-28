#![cfg(unix)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::time::{Duration, Instant};
use sysprims_timeout::{ContainmentCompletionEvidence, TerminateTreeConfig, TreeKillReliability};

fn require_disposable_container() {
    assert_eq!(
        std::env::var("PORTABLE_PTY_DISPOSABLE").as_deref(),
        Ok("1"),
        "diabolical tests require the disposable runner"
    );
    assert!(
        std::path::Path::new("/.dockerenv").exists(),
        "diabolical tests must not run on the host"
    );
}

fn escalation_config() -> TerminateTreeConfig {
    TerminateTreeConfig {
        grace_timeout_ms: 50,
        kill_timeout_ms: 2_000,
        ..TerminateTreeConfig::default()
    }
}

fn read_until(reader: &mut dyn Read, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        output.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&output);
        if text.contains(needle) {
            return text.into_owned();
        }
        assert!(
            count > 0 && Instant::now() < deadline,
            "child output: {}",
            text
        );
    }
}

#[test]
#[ignore = "requires the disposable container runner"]
fn term_resistant_descendants_require_bounded_escalation() {
    require_disposable_container();
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut command = CommandBuilder::new("/usr/bin/perl");
    command.args([
        "-e",
        "$SIG{TERM}='IGNORE'; for (1..3) { my $pid=fork(); die unless defined $pid; if (!$pid) { $SIG{TERM}='IGNORE'; sleep 60; exit 0; } } $|=1; print \"READY\\n\"; while (wait() > 0) {}",
    ]);

    let mut guard = pair.slave.spawn_contained_command(command).unwrap();
    assert_eq!(
        guard.tree_kill_reliability(),
        TreeKillReliability::Guaranteed
    );
    let output = read_until(&mut reader, "READY");
    assert!(output.contains("READY"));

    let outcome = guard.terminate(escalation_config()).unwrap();
    assert!(outcome.exited);
    assert!(outcome.escalated);
    assert!(matches!(
        outcome.completion,
        ContainmentCompletionEvidence::Empty { .. }
    ));
    assert!(guard.into_child().is_ok());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the disposable container runner"]
fn descendant_session_escape_does_not_upgrade_guarantee_to_non_escape() {
    require_disposable_container();
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut command = CommandBuilder::new("/usr/bin/perl");
    command.args([
        "-MPOSIX=setsid",
        "-e",
        "my $pid=fork(); die unless defined $pid; if (!$pid) { setsid() >= 0 or die \"setsid\"; $|=1; print \"ESCAPED $$\\n\"; sleep 60; exit 0; } $|=1; print \"READY\\n\"; waitpid($pid, 0)",
    ]);

    let mut guard = pair.slave.spawn_contained_command(command).unwrap();
    assert_eq!(
        guard.tree_kill_reliability(),
        TreeKillReliability::Guaranteed
    );
    let output = read_until(&mut reader, "ESCAPED");
    let escaped_pid = output
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|fields| (fields[0] == "ESCAPED").then(|| fields[1].parse::<u32>().unwrap()))
        .expect("escaped child pid missing");

    let outcome = guard.terminate(escalation_config()).unwrap();
    assert!(outcome.exited);
    assert!(
        std::path::Path::new(&format!("/proc/{escaped_pid}")).exists(),
        "cooperative escape scene did not escape the acquired group"
    );
    assert!(guard.into_child().is_ok());

    // The escaped process is intentionally not signaled from this test. The
    // disposable container boundary owns and destroys it after the suite.
}
