use super::Command;
use crate::interpreter::shell_os::vfs::{Vfs, Node};
use crate::interpreter::shell_os::ops_log::log_op;
use crate::interpreter::shell_os::cli::confirm;
use crate::interpreter::shell_os::vfs::path;

/// Register filesystem commands
pub fn register_fs_commands() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(Ls),
        Box::new(Cd),
        Box::new(Mkdir),
        Box::new(Rm),
        Box::new(Cp),
        Box::new(Mv),
    ]
}

pub struct Ls;
impl Command for Ls {
    fn name(&self) -> &'static str { "ls" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let path = args.get(0).copied().unwrap_or(".");
        let node = vfs.resolve(path)?;
        match node {
            Node::Dir(dir) => {
                for name in dir.children.keys() {
                    println!("{}", name);
                }
                Ok(())
            }
            Node::File(_) => {
                println!("{}", path);
                Ok(())
            }
        }
    }
}

pub struct Cd;
impl Command for Cd {
    fn name(&self) -> &'static str { "cd" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let path = args.get(0).copied().unwrap_or("/");
        let node = vfs.resolve(path)?;
        if !node.is_dir() {
            return Err("cd: not a directory".into());
        }
        vfs.cwd = path::normalize_path(&vfs.cwd, path);
        log_op("cd", path, "success");
        Ok(())
    }
}

pub struct Mkdir;
impl Command for Mkdir {
    fn name(&self) -> &'static str { "mkdir" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let Some(path) = args.get(0) else { return Err("mkdir: missing operand".into()); };
        if !confirm(&format!("mkdir: create directory '{}'?", path)) {
            println!("Aborted.");
            log_op("mkdir", path, "aborted");
            return Ok(());
        }
        let (parent, name) = vfs.resolve_parent_mut(path)?;
        if parent.contains_key(&name) {
            log_op("mkdir", path, "exists");
            return Err("mkdir: already exists".into());
        }
        parent.insert(name, Node::new_dir());
        match vfs.save() {
            Ok(_) => {
                log_op("mkdir", path, "success");
                Ok(())
            }
            Err(e) => {
                log_op("mkdir", path, &format!("error: {}", e));
                Err(e)
            }
        }
    }
}

pub struct Rm;
impl Command for Rm {
    fn name(&self) -> &'static str { "rm" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        let Some(path) = args.get(0) else { return Err("rm: missing operand".into()); };
        if !confirm(&format!("rm: delete '{}'?", path)) {
            println!("Aborted.");
            log_op("rm", path, "aborted");
            return Ok(());
        }
        let (parent, name) = vfs.resolve_parent_mut(path)?;
        if parent.remove(&name).is_none() {
            log_op("rm", path, "not_found");
            return Err("rm: not found".into());
        }
        match vfs.save() {
            Ok(_) => {
                log_op("rm", path, "success");
                Ok(())
            }
            Err(e) => {
                log_op("rm", path, &format!("error: {}", e));
                Err(e)
            }
        }
    }
}

pub struct Cp;
impl Command for Cp {
    fn name(&self) -> &'static str { "cp" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.len() < 2 { return Err("cp: missing operands".into()); }
        let src = args[0];
        let dst = args[1];
        if !confirm(&format!("cp: copy '{}' to '{}'?", src, dst)) {
            println!("Aborted.");
            log_op("cp", &format!("{} -> {}", src, dst), "aborted");
            return Ok(());
        }
        let src_node = vfs.resolve(src)?.clone();
        let (parent, name) = vfs.resolve_parent_mut(dst)?;
        parent.insert(name, src_node);
        match vfs.save() {
            Ok(_) => {
                log_op("cp", &format!("{} -> {}", src, dst), "success");
                Ok(())
            }
            Err(e) => {
                log_op("cp", &format!("{} -> {}", src, dst), &format!("error: {}", e));
                Err(e)
            }
        }
    }
}

pub struct Mv;
impl Command for Mv {
    fn name(&self) -> &'static str { "mv" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.len() < 2 { return Err("mv: missing operands".into()); }
        let src = args[0];
        let dst = args[1];
        if !confirm(&format!("mv: move '{}' to '{}'?", src, dst)) {
            println!("Aborted.");
            log_op("mv", &format!("{} -> {}", src, dst), "aborted");
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
        match vfs.save() {
            Ok(_) => {
                log_op("mv", &format!("{} -> {}", src, dst), "success");
                Ok(())
            }
            Err(e) => {
                log_op("mv", &format!("{} -> {}", src, dst), &format!("error: {}", e));
                Err(e)
            }
        }
    }
}
