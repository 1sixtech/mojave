pub mod error;
pub use error::DaemonError;

use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use daemonize::Daemonize;
use sysinfo::{Pid, System};

pub struct DaemonOptions {
    pub no_daemon: bool,
    pub pid_file_path: PathBuf,
    pub log_file_path: PathBuf,
}

pub async fn run_daemonized<F, Fut>(opts: DaemonOptions, proc: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    if opts.no_daemon {
        run_main_task(proc).await;
        return Ok(());
    }

    let log_path = resolve_path(&opts.log_file_path)?;
    let pid_path = resolve_path(&opts.pid_file_path)?;

    match read_pid_from_file(&pid_path) {
        Ok(pid) if is_pid_running(pid) => return Err(DaemonError::AlreadyRunning(pid).into()),
        _ => {
            let _ = std::fs::remove_file(&pid_path);
        }
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

    let daemon = Daemonize::new()
        .pid_file(pid_path.clone())
        .chown_pid_file(true)
        .stdout(log_file)
        .stderr(log_file_err);
    daemon.start()?;

    run_main_task(proc).await;
    let _ = std::fs::remove_file(pid_path);

    Ok(())
}

pub fn stop_daemonized<P: AsRef<Path>>(pid_file: P) -> Result<(), DaemonError> {
    let pid_file = resolve_path(pid_file)?;
    let pid = read_pid_from_file(pid_file)?;

    let system = System::new_all();
    match system.process(pid) {
        Some(process) => {
            process.kill();
            Ok(())
        }
        None => Err(DaemonError::NoSuchProcess(pid)),
    }
}

fn resolve_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, DaemonError> {
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

fn read_pid_from_file<P: AsRef<Path>>(path: P) -> Result<Pid, DaemonError> {
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

async fn run_main_task<F, Fut>(proc: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    tokio::select! {
        res = proc() => {
            if let Err(err) = res {
                tracing::error!("Process stopped unexpectedly: {}", err);
            }
        },
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down...");
        }
    }
}
