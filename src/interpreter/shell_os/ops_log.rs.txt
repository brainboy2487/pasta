use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::env;
use chrono::Utc;

/// Return the per-user ops log path: $HOME/.local/share/shell_os/ops.log
fn ops_log_path() -> PathBuf {
    if let Ok(p) = env::var("SHELL_OS_OPS_LOG") {
        return PathBuf::from(p);
    }
    let mut p = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    p.push(".local");
    p.push("share");
    p.push("shell_os");
    let _ = std::fs::create_dir_all(&p);
    p.push("ops.log");
    p
}

/// Append a single operation record. Best-effort: errors are ignored.
pub fn log_op(cmd: &str, target: &str, result: &str) {
    let path = ops_log_path();
    let ts = Utc::now().to_rfc3339();
    let user = env::var("USER").unwrap_or_else(|_| "unknown".into());
    let line = format!("{} | user={} | cmd={} | target={} | result={}\n", ts, user, cmd, target, result);

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}
