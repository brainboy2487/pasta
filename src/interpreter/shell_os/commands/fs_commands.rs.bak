use super::Command;
use crate::interpreter::shell_os::vfs::{Vfs, Node};
use crate::interpreter::shell_os::ops_log::log_op;
use crate::interpreter::shell_os::cli::confirm;
use crate::interpreter::shell_os::vfs::path;
use std::path::PathBuf;

/// Register all filesystem commands.
pub fn register_fs_commands() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(Ls),
        Box::new(Cd),
        Box::new(Pwd),
        Box::new(Mkdir),
        Box::new(Touch),
        Box::new(Cat),
        Box::new(Rm),
        Box::new(Cp),
        Box::new(Mv),
        Box::new(ChangeFs),
    ]
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn local_path(vfs: &Vfs, target: &str) -> PathBuf {
    let t = PathBuf::from(target);
    if t.is_absolute() { t } else { vfs.local_cwd.join(target) }
}

// ── ls ───────────────────────────────────────────────────────────────────────

pub struct Ls;
impl Command for Ls {
    fn name(&self) -> &'static str { "ls" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if vfs.local_mode {
            let dir = args.first().map(|a| local_path(vfs, a))
                .unwrap_or_else(|| vfs.local_cwd.clone());
            let mut entries: Vec<String> = std::fs::read_dir(&dir)
                .map_err(|e| format!("ls: {}", e))?
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            entries.sort();
            for name in entries { println!("{}", name); }
            return Ok(());
        }
        let target = args.first().copied().unwrap_or(".");
        let node = vfs.resolve(target)?;
        match node {
            Node::Dir(dir) => {
                let mut names: Vec<&String> = dir.children.keys().collect();
                names.sort();
                for name in names { println!("{}", name); }
            }
            Node::File(_) => println!("{}", target),
        }
        Ok(())
    }
}

// ── cd ───────────────────────────────────────────────────────────────────────

pub struct Cd;
impl Command for Cd {
    fn name(&self) -> &'static str { "cd" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if vfs.local_mode {
            let target = args.first().copied().unwrap_or("/");
            let new_path = if target == "/" {
                PathBuf::from("/")
            } else {
                local_path(vfs, target)
            };
            if !new_path.is_dir() {
                return Err(format!("cd: not a directory: {}", target));
            }
            vfs.local_cwd = new_path.canonicalize()
                .unwrap_or(new_path);
            log_op("cd", target, "success");
            return Ok(());
        }
        let target = args.first().copied().unwrap_or("/");
        let node = vfs.resolve(target)?;
        if !node.is_dir() {
            return Err(format!("cd: not a directory: {}", target));
        }
        vfs.cwd = path::normalize_path(&vfs.cwd, target);
        log_op("cd", target, "success");
        Ok(())
    }
}

// ── pwd ──────────────────────────────────────────────────────────────────────

pub struct Pwd;
impl Command for Pwd {
    fn name(&self) -> &'static str { "pwd" }
    fn run(&self, _args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        println!("{}", vfs.prompt_path());
        Ok(())
    }
}

// ── mkdir ─────────────────────────────────────────────────────────────────────

pub struct Mkdir;
impl Command for Mkdir {
    fn name(&self) -> &'static str { "mkdir" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let Some(target) = args.first() else {
            return Err("mkdir: missing operand".into());
        };
        if !confirm(&format!("mkdir: create directory '{}'?", target)) {
            println!("Aborted.");
            log_op("mkdir", target, "aborted");
            return Ok(());
        }
        if vfs.local_mode {
            let full = local_path(vfs, target);
            std::fs::create_dir_all(&full)
                .map_err(|e| format!("mkdir: {}", e))?;
            log_op("mkdir", target, "success");
            println!("Created directory: {}", full.display());
            return Ok(());
        }
        let (parent, name) = vfs.resolve_parent_mut(target)?;
        if parent.contains_key(&name) {
            log_op("mkdir", target, "exists");
            return Err(format!("mkdir: already exists: {}", target));
        }
        parent.insert(name, Node::new_dir());
        vfs.save().map_err(|e| { log_op("mkdir", target, &format!("error: {}", e)); e })?;
        log_op("mkdir", target, "success");
        println!("Created directory: {}", target);
        Ok(())
    }
}

// ── touch ─────────────────────────────────────────────────────────────────────

pub struct Touch;
impl Command for Touch {
    fn name(&self) -> &'static str { "touch" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let Some(target) = args.first() else {
            return Err("touch: missing operand".into());
        };
        if vfs.local_mode {
            let full = local_path(vfs, target);
            std::fs::OpenOptions::new().create(true).write(true).open(&full)
                .map_err(|e| format!("touch: {}", e))?;
            log_op("touch", target, "success");
            return Ok(());
        }
        let (parent, name) = vfs.resolve_parent_mut(target)?;
        if !parent.contains_key(&name) {
            parent.insert(name, Node::new_file(Vec::new()));
            vfs.save().map_err(|e| { log_op("touch", target, &format!("error: {}", e)); e })?;
        }
        log_op("touch", target, "success");
        Ok(())
    }
}

// ── cat ───────────────────────────────────────────────────────────────────────

pub struct Cat;
impl Command for Cat {
    fn name(&self) -> &'static str { "cat" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.is_empty() {
            return Err("cat: missing operand".into());
        }
        for target in args {
            if vfs.local_mode {
                let full = local_path(vfs, target);
                let content = std::fs::read_to_string(&full)
                    .map_err(|e| format!("cat: {}: {}", target, e))?;
                print!("{}", content);
                continue;
            }
            let node = vfs.resolve(target)?;
            match node {
                Node::File(f) => match std::str::from_utf8(&f.data) {
                    Ok(s) => print!("{}", s),
                    Err(_) => println!("[binary file: {} bytes]", f.data.len()),
                },
                Node::Dir(_) => return Err(format!("cat: {}: is a directory", target)),
            }
        }
        println!();
        Ok(())
    }
}

// ── rm ────────────────────────────────────────────────────────────────────────

pub struct Rm;
impl Command for Rm {
    fn name(&self) -> &'static str { "rm" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let Some(target) = args.first() else {
            return Err("rm: missing operand".into());
        };
        if !confirm(&format!("rm: delete '{}'?", target)) {
            println!("Aborted.");
            log_op("rm", target, "aborted");
            return Ok(());
        }
        if vfs.local_mode {
            let full = local_path(vfs, target);
            if full.is_dir() {
                std::fs::remove_dir_all(&full)
            } else {
                std::fs::remove_file(&full)
            }.map_err(|e| format!("rm: {}", e))?;
            log_op("rm", target, "success");
            println!("Removed: {}", full.display());
            return Ok(());
        }
        let (parent, name) = vfs.resolve_parent_mut(target)?;
        if parent.remove(&name).is_none() {
            log_op("rm", target, "not_found");
            return Err(format!("rm: not found: {}", target));
        }
        vfs.save().map_err(|e| { log_op("rm", target, &format!("error: {}", e)); e })?;
        log_op("rm", target, "success");
        println!("Removed: {}", target);
        Ok(())
    }
}

// ── cp ────────────────────────────────────────────────────────────────────────

pub struct Cp;
impl Command for Cp {
    fn name(&self) -> &'static str { "cp" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.len() < 2 { return Err("cp: missing operands".into()); }
        let (src, dst) = (args[0], args[1]);
        if !confirm(&format!("cp: copy '{}' to '{}'?", src, dst)) {
            println!("Aborted.");
            log_op("cp", &format!("{} -> {}", src, dst), "aborted");
            return Ok(());
        }
        if vfs.local_mode {
            let s = local_path(vfs, src);
            let d = local_path(vfs, dst);
            std::fs::copy(&s, &d).map_err(|e| format!("cp: {}", e))?;
            log_op("cp", &format!("{} -> {}", src, dst), "success");
            return Ok(());
        }
        let src_node = vfs.resolve(src)?.clone();
        let (parent, name) = vfs.resolve_parent_mut(dst)?;
        parent.insert(name, src_node);
        vfs.save().map_err(|e| { log_op("cp", &format!("{}->{}", src, dst), &format!("error: {}", e)); e })?;
        log_op("cp", &format!("{} -> {}", src, dst), "success");
        Ok(())
    }
}

// ── mv ────────────────────────────────────────────────────────────────────────

pub struct Mv;
impl Command for Mv {
    fn name(&self) -> &'static str { "mv" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.len() < 2 { return Err("mv: missing operands".into()); }
        let (src, dst) = (args[0], args[1]);
        if !confirm(&format!("mv: move '{}' to '{}'?", src, dst)) {
            println!("Aborted.");
            log_op("mv", &format!("{} -> {}", src, dst), "aborted");
            return Ok(());
        }
        if vfs.local_mode {
            let s = local_path(vfs, src);
            let d = local_path(vfs, dst);
            std::fs::rename(&s, &d).map_err(|e| format!("mv: {}", e))?;
            log_op("mv", &format!("{} -> {}", src, dst), "success");
            return Ok(());
        }
        let src_node = vfs.resolve(src)?.clone();
        {
            let (src_parent, src_name) = vfs.resolve_parent_mut(src)?;
            src_parent.remove(&src_name)
                .ok_or_else(|| "mv: source not found".to_string())?;
        }
        let (dst_parent, dst_name) = vfs.resolve_parent_mut(dst)?;
        dst_parent.insert(dst_name, src_node);
        vfs.save().map_err(|e| { log_op("mv", &format!("{}->{}", src, dst), &format!("error: {}", e)); e })?;
        log_op("mv", &format!("{} -> {}", src, dst), "success");
        Ok(())
    }
}

// ── change_fs ─────────────────────────────────────────────────────────────────

pub struct ChangeFs;
impl Command for ChangeFs {
    fn name(&self) -> &'static str { "change_fs" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        use std::io::{self, Write};

        let input: String = if let Some(p) = args.first() {
            p.to_string()
        } else {
            print!("Enter filesystem ('local' or image path): ");
            io::stdout().flush().ok();
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)
                .map_err(|e| format!("read error: {}", e))?;
            buf.trim().to_string()
        };

        if input.is_empty() {
            return Err("change_fs: no input provided".into());
        }

        if input == "local" {
            vfs.mount_local();
            println!("Mounted local filesystem at {}", vfs.local_cwd.display());
            log_op("change_fs", "local", "success");
        } else {
            // Treat "fs.img" shorthand as the default image path
            let img_path = if input == "fs.img" {
                PathBuf::from(crate::interpreter::shell_os::vfs::fs::DEFAULT_FS_IMAGE)
            } else {
                PathBuf::from(&input)
            };
            println!("Saving and remounting {}...", img_path.display());
            vfs.mount(img_path.clone())
                .map_err(|e| format!("change_fs: {}", e))?;
            println!("Mounted: {}", img_path.display());
            log_op("change_fs", &img_path.to_string_lossy(), "success");
        }
        Ok(())
    }
}
