use crate::cmdbuilder::CommandBuilder;
use crate::win::psuedocon::PsuedoCon;
use crate::{
    Child, ContainedPtyChild, ContainedPtyGuard, ContainedPtySpawnError,
    ContainedPtySpawnErrorStage, FailedChildRecovery, MasterPty, PtyPair, PtySize, PtySystem,
    SlavePty,
};
use anyhow::Error;
use filedescriptor::{FileDescriptor, Pipe};
use std::sync::{Arc, Mutex};
use sysprims_timeout::{PreparedWindowsJob, TerminateTreeConfig};
use winapi::um::wincon::COORD;

#[derive(Default)]
pub struct ConPtySystem {}

fn open_conpty(size: PtySize) -> anyhow::Result<(ConPtyMasterPty, ConPtySlavePty)> {
    let stdin = Pipe::new()?;
    let stdout = Pipe::new()?;
    let con = PsuedoCon::new(
        COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        },
        stdin.read,
        stdout.write,
    )?;
    let master = ConPtyMasterPty {
        inner: Arc::new(Mutex::new(Inner {
            con,
            readable: stdout.read,
            writable: Some(stdin.write),
            size,
        })),
    };
    let slave = ConPtySlavePty {
        inner: master.inner.clone(),
    };
    Ok((master, slave))
}

impl PtySystem for ConPtySystem {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair> {
        let (master, slave) = open_conpty(size)?;
        Ok(PtyPair {
            master: Box::new(master),
            slave: Box::new(slave),
        })
    }
}

struct Inner {
    con: PsuedoCon,
    readable: FileDescriptor,
    writable: Option<FileDescriptor>,
    size: PtySize,
}

impl Inner {
    pub fn resize(
        &mut self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.con.resize(COORD {
            X: num_cols as i16,
            Y: num_rows as i16,
        })?;
        self.size = PtySize {
            rows: num_rows,
            cols: num_cols,
            pixel_width,
            pixel_height,
        };
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConPtyMasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct ConPtySlavePty {
    inner: Arc<Mutex<Inner>>,
}

impl ConPtySlavePty {
    fn spawn_contained_command_inner<F>(
        &self,
        cmd: CommandBuilder,
        before_resume: F,
    ) -> Result<ContainedPtyGuard, ContainedPtySpawnError>
    where
        F: FnOnce(&ContainedPtyGuard) -> Result<(), Error>,
    {
        let prepared_job = PreparedWindowsJob::new()
            .map_err(|error| ContainedPtySpawnError::before_spawn(error.into()))?;
        let recovery = FailedChildRecovery::prepare()
            .map_err(|error| ContainedPtySpawnError::before_spawn(error.into()))?;
        let inner = self.inner.lock().unwrap();
        let (child, primary_thread) = inner
            .con
            .spawn_suspended_command(cmd)
            .map_err(ContainedPtySpawnError::before_spawn)?;
        let child = ContainedPtyChild::new(child);
        let process = Child::as_raw_handle(&child)
            .expect("Windows contained child retains its process handle");

        // SAFETY: this handle and primary thread came from the one suspended
        // CreateProcessW call above. The thread has not been resumed.
        let receipt = match unsafe { prepared_job.assign_process(process) } {
            Ok(receipt) => receipt,
            Err(error) => {
                drop(primary_thread);
                return Err(ContainedPtySpawnError::after_spawn(
                    ContainedPtySpawnErrorStage::Receipt,
                    error.into(),
                    child,
                    recovery,
                ));
            }
        };

        // SAFETY: child owns the exact process handle sealed into receipt,
        // remains suspended, and transfers exclusive wait/reap authority.
        let mut guard =
            match unsafe { sysprims_timeout::contain_acquired_windows_job(child, receipt) } {
                Ok(guard) => guard,
                Err(adoption) => {
                    drop(primary_thread);
                    return Err(ContainedPtySpawnError::after_spawn(
                        ContainedPtySpawnErrorStage::Adoption,
                        adoption.error.into(),
                        adoption.child,
                        recovery,
                    ));
                }
            };

        if let Err(error) = before_resume(&guard) {
            drop(primary_thread);
            let _ = guard.terminate(TerminateTreeConfig {
                grace_timeout_ms: 0,
                kill_timeout_ms: 2_000,
                ..TerminateTreeConfig::default()
            });
            return Err(ContainedPtySpawnError {
                stage: ContainedPtySpawnErrorStage::Resume,
                source: error,
                recovery: None,
            });
        }

        if let Err(error) = primary_thread.resume() {
            // The child was never made runnable by this adapter. Resolve it
            // through the already-owned Job and return no active guard.
            let _ = guard.terminate(TerminateTreeConfig {
                grace_timeout_ms: 0,
                kill_timeout_ms: 2_000,
                ..TerminateTreeConfig::default()
            });
            return Err(ContainedPtySpawnError {
                stage: ContainedPtySpawnErrorStage::Resume,
                source: error.into(),
                recovery: None,
            });
        }

        Ok(guard)
    }
}

impl MasterPty for ConPtyMasterPty {
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.resize(size.rows, size.cols, size.pixel_width, size.pixel_height)
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.size)
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(self.inner.lock().unwrap().readable.try_clone()?))
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        Ok(Box::new(
            self.inner
                .lock()
                .unwrap()
                .writable
                .take()
                .ok_or_else(|| anyhow::anyhow!("writer already taken"))?,
        ))
    }
}

impl SlavePty for ConPtySlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Box<dyn Child + Send + Sync>> {
        let inner = self.inner.lock().unwrap();
        let child = inner.con.spawn_command(cmd)?;
        Ok(Box::new(child))
    }

    fn spawn_contained_command(
        &self,
        cmd: CommandBuilder,
    ) -> Result<ContainedPtyGuard, ContainedPtySpawnError> {
        self.spawn_contained_command_inner(cmd, |_| Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use sysprims_timeout::{ContainmentBoundaryStrength, TreeKillReliability};

    fn marker_path(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sysprims-pty-{label}-{}-{nonce}",
            std::process::id(),
        ))
    }

    #[test]
    fn child_cannot_run_before_assignment_proof_and_resume() {
        let marker = marker_path("resume-sentinel");
        let _ = std::fs::remove_file(&marker);
        let (master, slave) = open_conpty(PtySize::default()).expect("ConPTY open failed");
        let mut command = CommandBuilder::new("cmd.exe");
        command.args([
            "/C",
            &format!(
                "echo started>\"{}\" & ping -n 30 127.0.0.1 >NUL",
                marker.display()
            ),
        ]);

        let mut guard = slave
            .spawn_contained_command_inner(command, |guard| {
                assert_eq!(
                    guard.tree_kill_reliability(),
                    TreeKillReliability::Guaranteed
                );
                assert_eq!(
                    guard.boundary_strength(),
                    ContainmentBoundaryStrength::KernelEnforcedJob
                );
                std::thread::sleep(Duration::from_millis(100));
                assert!(
                    !marker.exists(),
                    "suspended child executed before the explicit resume gate"
                );
                Ok(())
            })
            .expect("contained ConPTY spawn failed");

        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() {
            assert!(Instant::now() < deadline, "resumed child did not execute");
            std::thread::sleep(Duration::from_millis(10));
        }
        master
            .resize(PtySize {
                rows: 30,
                cols: 100,
                ..PtySize::default()
            })
            .expect("presentation handle was not retained");
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

    #[test]
    fn failed_resume_gate_never_runs_child() {
        let marker = marker_path("failed-resume");
        let _ = std::fs::remove_file(&marker);
        let (_master, slave) = open_conpty(PtySize::default()).expect("ConPTY open failed");
        let mut command = CommandBuilder::new("cmd.exe");
        command.args(["/C", &format!("echo started>\"{}\"", marker.display())]);

        let error = match slave.spawn_contained_command_inner(command, |_| {
            Err(anyhow::anyhow!("injected resume-gate failure"))
        }) {
            Ok(_) => panic!("resume-gate failure must fail the transaction"),
            Err(error) => error,
        };
        assert_eq!(error.stage(), ContainedPtySpawnErrorStage::Resume);
        assert!(!error.recovery_pending());
        assert!(
            !marker.exists(),
            "failed resume transaction executed the child"
        );
    }
}
