use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::Result;
use daemonize::Daemonize;
use sysinfo::{Pid, System};
use thiserror::Error;

const PROCESS_KILL_TIMEOUT_SEC: u64 = 5;

pub struct DaemonOptions {
    pub no_daemon: bool,
    pub pid_file_path: PathBuf,
    pub log_file_path: PathBuf,
}

type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("pid in pid file is already running. pid: {0}")]
    AlreadyRunning(Pid),

    #[error("daemonize failed: {0}")]
    Daemonize(#[from] daemonize::Error),

    #[error("I/O error at {path}: {source}")]
    IoWithPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no such process with pid: {0}")]
    NoSuchProcess(Pid),

    #[error("failed to parse pid from '{0}': expected integer")]
    ParsePid(String),
}

pub fn run_daemonized<F, Fut>(opts: DaemonOptions, proc: F) -> Result<(), DynError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), DynError>>,
{
    if opts.no_daemon {
        return run_main_task(proc);
    }

    let log_path = resolve_path(&opts.log_file_path)?;
    let pid_path = resolve_path(&opts.pid_file_path)?;

    if let Some(pid) = read_pid_from_file(&pid_path)
        .ok()
        .filter(|pid| is_pid_running(pid.to_owned()))
    {
        return Err(DaemonError::AlreadyRunning(pid).into());
    }

    match std::fs::remove_file(&pid_path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!(
                ?pid_path,
                error = %e,
                "Failed to remove stale pid file while preparing daemon"
            );
        }
        _ => {}
    }
    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&log_path)
        .map_err(|source| DaemonError::IoWithPath {
            path: log_path.clone(),
            source,
        })?;
    let log_file_err = log_file
        .try_clone()
        .map_err(|source| DaemonError::IoWithPath {
            path: log_path.clone(),
            source,
        })?;

    let working_dir = std::env::current_dir()?;

    let daemon = Daemonize::new()
        .pid_file(pid_path.clone())
        .chown_pid_file(true)
        .umask(0o027)
        .working_directory(working_dir)
        .stdout(log_file)
        .stderr(log_file_err);
    daemon.start()?;

    if let Err(e) = run_main_task(proc) {
        tracing::error!("run_main_task failed: {e}");
        return Err(e);
    }

    Ok(())
}

pub fn stop_daemonized<P: AsRef<Path>>(pid_file: P) -> Result<()> {
    let pid_file = resolve_path(pid_file)?;
    let pid = read_pid_from_file(&pid_file)?;

    let system = System::new_all();
    match system.process(pid) {
        Some(process) => {
            process.kill_with(sysinfo::Signal::Interrupt);
            let start_time = std::time::Instant::now();
            let time_out = Duration::from_secs(PROCESS_KILL_TIMEOUT_SEC);
            while start_time.elapsed() < time_out {
                if !is_pid_running(pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            if is_pid_running(pid) {
                process.kill();
            }

            if let Err(e) = std::fs::remove_file(pid_file) {
                tracing::warn!(error = %e, "Failed to remove pid file after stopping process");
            }
            Ok(())
        }
        None => Err(DaemonError::NoSuchProcess(pid).into()),
    }
}

fn resolve_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    if path.as_ref().is_absolute() {
        return Ok(path.as_ref().to_path_buf());
    }

    let path_buf = match std::env::home_dir() {
        Some(home) => home.join(path),
        None => PathBuf::from(".").join(path),
    };

    if let Some(parent) = path_buf.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent).map_err(|source| DaemonError::IoWithPath {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(path_buf)
}

fn read_pid_from_file<P: AsRef<Path>>(path: P) -> Result<Pid> {
    let content =
        std::fs::read_to_string(path.as_ref()).map_err(|source| DaemonError::IoWithPath {
            path: path.as_ref().to_path_buf(),
            source,
        })?;
    let trimmed = content.trim();
    let pid = Pid::from_str(trimmed).map_err(|_| DaemonError::ParsePid(trimmed.to_owned()))?;
    Ok(pid)
}

fn is_pid_running(pid: Pid) -> bool {
    System::new_all().process(pid).is_some()
}

fn run_main_task<F, Fut>(proc: F) -> Result<(), DynError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), DynError>>,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        let res = proc().await;
        if let Err(err) = res {
            tracing::error!("Process stopped unexpectedly: {}", err);
            return Err(err);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    /// create temporary directory path
    fn unique_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!(
            "mojave_daemon_test_{}_{}_{}",
            name,
            std::process::id(),
            ts
        ));
        p
    }

    #[test]
    fn resolve_path_keeps_absolute_path() {
        let abs = unique_path("abs_dir").join("pidfile.pid");
        let resolved = resolve_path(&abs).unwrap();
        assert_eq!(resolved, abs);
    }

    #[test]
    fn read_pid_from_file_parses_current_pid() {
        // Write the current process PID to a file and check if it is read correctly + if it is a running PID
        let pid_file = unique_path("pid_ok");
        fs::create_dir_all(pid_file.parent().unwrap()).unwrap();
        let my_pid_str = format!("{}", std::process::id());
        fs::write(&pid_file, &my_pid_str).unwrap();

        let pid = read_pid_from_file(&pid_file).unwrap();

        assert!(is_pid_running(pid), "current process should be running");

        // cleanup temp test file
        let _ = fs::remove_file(pid_file);
    }

    #[test]
    fn read_pid_from_file_parse_error() {
        let pid_file = unique_path("pid_parse_err");
        fs::create_dir_all(pid_file.parent().unwrap()).unwrap();
        fs::write(&pid_file, "not-an-int\n").unwrap();

        let err = read_pid_from_file(&pid_file).unwrap_err();

        assert!(matches!(
            err.downcast_ref::<DaemonError>(),
            Some(DaemonError::ParsePid(_))
        ));
    }

    #[test]
    fn read_pid_from_file_io_error_when_missing() {
        // if no such file, return IoWithPath error
        let missing = unique_path("pid_missing");
        let err = read_pid_from_file(&missing).unwrap_err();

        assert!(matches!(
            err.downcast_ref::<DaemonError>(),
            Some(DaemonError::IoWithPath { .. })
        ));
    }

    #[test]
    fn run_daemonized_sync_no_daemon_ok() {
        let opts = DaemonOptions {
            no_daemon: true,
            pid_file_path: unique_path("unused_pid3"),
            log_file_path: unique_path("unused_log3"),
        };
        let res = run_daemonized(opts, || async { Ok(()) });

        assert!(res.is_ok());
    }

    #[test]
    fn run_daemonized_sync_no_daemon_err_propagates() {
        let opts = DaemonOptions {
            no_daemon: true,
            pid_file_path: unique_path("unused_pid4"),
            log_file_path: unique_path("unused_log4"),
        };
        let res = run_daemonized(opts, || async { Err::<(), _>("propagate".into()) });

        assert!(res.is_err());
        assert!(format!("{res:#?}").contains("propagate"));
    }

    #[tokio::test]
    async fn stop_daemonized_returns_no_such_process_for_fake_pid() {
        let pid_file = unique_path("fake_pid");
        fs::create_dir_all(pid_file.parent().unwrap()).unwrap();
        fs::write(&pid_file, "0").unwrap();

        let err = stop_daemonized(&pid_file).unwrap_err();

        assert!(matches!(
            err.downcast_ref::<DaemonError>(),
            Some(DaemonError::NoSuchProcess(_))
        ));

        // In actual usage, the pid file is not removed if process is not found
        let _ = fs::remove_file(pid_file);
    }
}
