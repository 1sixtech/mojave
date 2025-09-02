use std::path::PathBuf;

use sysinfo::Pid;
use thiserror::Error;

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
