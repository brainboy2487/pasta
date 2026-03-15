use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn write_snapshot_atomic(
    tmp_dir: &Path,
    outbox_dir: &Path,
    json_bytes: &[u8],
    task_id: &str,
    epoch: u64,
    seq: u64,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(tmp_dir)?;
    fs::create_dir_all(outbox_dir)?;
    let tmp_name = format!("tmp-{}-{}.json", task_id, seq);
    let tmp_path = tmp_dir.join(&tmp_name);
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(json_bytes)?;
        f.sync_all()?;
    }
    let out_name = format!("task-{}-epoch{}-seq{}.json", task_id, epoch, seq);
    let out_path = outbox_dir.join(&out_name);
    fs::rename(&tmp_path, &out_path)?;
    Ok(out_path)
}

pub fn claim_snapshot(outbox_dir: &Path, inflight_dir: &Path) -> std::io::Result<Option<PathBuf>> {
    fs::create_dir_all(inflight_dir)?;
    for entry in fs::read_dir(outbox_dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let filename = p.file_name().unwrap().to_string_lossy().to_string();
            let inflight_path = inflight_dir.join(&filename);
            // Attempt atomic rename to claim
            match fs::rename(&p, &inflight_path) {
                Ok(_) => return Ok(Some(inflight_path)),
                Err(_) => continue,
            }
        }
    }
    Ok(None)
}
