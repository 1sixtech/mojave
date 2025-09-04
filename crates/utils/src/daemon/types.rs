use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonOptions {
    pub no_daemon: bool,
    pub pid_file_path: PathBuf,
    pub log_file_path: PathBuf,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        DaemonOptions {
            no_daemon: true,
            pid_file_path: Default::default(),
            log_file_path: Default::default(),
        }
    }
}

fn apply_daemon_partial(base: &mut DaemonOptions, p: &DaemonPartialOptions, datadir: &str) {
    if base.pid_file_path == PathBuf::new() {
        base.pid_file_path = {
            let mut path = PathBuf::from(datadir);
            path.push("sequencer.pid");
            path
        };
    }
    if base.log_file_path == PathBuf::new() {
        base.log_file_path = {
            let mut path = PathBuf::from(datadir);
            path.push("sequencer.log");
            path
        };
    }

    if p.no_daemon {
        base.no_daemon = true;
    }
    if let Some(v) = &p.pid_file {
        base.pid_file_path = v.clone();
    }
    if let Some(v) = &p.log_file {
        base.log_file_path = v.clone();
    }
}
