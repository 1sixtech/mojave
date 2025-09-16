Mojave Daemonization
====================
  
This document explains how Mojave binaries (node, sequencer, prover) are daemonized, how PID/log files are handled, and how to start/stop services.
  
Overview
--------
All Mojave services share a common daemon helper implemented in `crates/utils/src/daemon.rs`. Each binary exposes `init` (start) and `stop` subcommands and constructs `DaemonOptions` with PID/log paths derived from the selected `--datadir` (using fixed per‑binary filenames), then hands it to the helper.
  
High‑level flow
---------------
The runtime follows this decision tree:
  
<img width="674" height="672" alt="image" src="https://github.com/user-attachments/assets/dec958ac-4cb0-4af6-bc47-f892b316c378" />
  
Key types and entry points
--------------------------
- `DaemonOptions` (paths derived from `--datadir` + fixed filenames, and `no_daemon` flag)
- `run_daemonized_async(opts, proc)` (Tokio main; used by node/sequencer)
- `run_daemonized(opts, proc)` (spawns a Tokio runtime and calls the async version; used by prover)
- `stop_daemonized(pid_file)` (SIGINT with timeout, then SIGKILL fallback, remove PID file)
  
Where they are used
-------------------
- `cmd/node/src/main.rs` uses `run_daemonized_async` and constructs `DaemonOptions` with `node.pid` / `node.log`.
- `cmd/sequencer/src/main.rs` uses `run_daemonized_async` with `sequencer.pid` / `sequencer.log`.
- `cmd/prover/src/main.rs` uses `run_daemonized` with `prover.pid` / `prover.log`.
  
CLI flags and commands
----------------------
- `init` (start): launches the service; by default runs as a daemon unless `--no-daemon` is supplied.
- `stop`: reads the PID from the PID file and stops the running service safely. Then remove PID file.
  
E.g.:
- `mojave-[node | sequencer | prover] init --no-daemon` (foreground) or omit to daemonize; `mojave-[node | sequencer | prover] stop` to stop. Datadir default: `.mojave/[node | sequencer | prover]`
  
PID/log file locations
----------------------
Each binary writes under its `--datadir` :
- Node: `node.pid`, `node.log`
- Sequencer: `sequencer.pid`, `sequencer.log`
- Prover: `prover.pid`, `prover.log`
Paths are resolved to absolute paths (home‑relative supported). Parent directories are created if missing.
 
Detailed behavior
-----------------
1) Preflight: If a PID file exists and the PID is running, startup fails with `AlreadyRunning(pid)`. If the PID file is stale, it is removed.
2) Daemonization: On daemon mode, the process forks using `daemonize::Daemonize`, sets `umask(0o027)`, preserves the current working directory, and redirects `stdout`/`stderr` to the log file.
3) Main task: The provided async closure is executed. On completion or error, the PID file is removed (best‑effort with a warning on failure), and errors are logged and bubbled up.
4) Stop: The `stop` subcommand sends `SIGINT` first and waits up to 5s for a clean exit. If still running, it sends a hard kill. The PID file is then removed.
 
Error handling
--------------
Common errors include:
- `AlreadyRunning(Pid)`: refuse to start when a live PID is detected.
- `IoWithPath { path, source }`: contextualized I/O errors for PID/log file operations.
- `ParsePid(..)`: PID file contained a non‑integer value.
- `NoSuchProcess(Pid)`: stop requested but the process does not exist.
  
Examples
--------
Start the node in the background (daemon):
```bash
mojave-node init
```
  
Start the sequencer in the foreground (no daemon):
```bash
mojave-sequencer init --no-daemon --private_key <HEX>
```
Stop the prover:
```bash
mojave-prover stop
```
  
Source references
-----------------
- `crates/utils/src/daemon.rs`
- `cmd/node/src/main.rs`, `cmd/node/src/cli.rs`
- `cmd/sequencer/src/main.rs`, `cmd/sequencer/src/cli.rs`
- `cmd/prover/src/main.rs`, `cmd/prover/src/cli.rs`
  
Notes
-----
- Logging level can be controlled via `--log.level` flags per binary.
- PID/log file names are fixed per binary; customize location by changing `--datadir`.
