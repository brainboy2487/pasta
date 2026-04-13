use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use crate::interpreter::shell_os::commands::{Command as ShellCommand, register_fs_commands};
use crate::interpreter::shell_os::vfs::Vfs;

/// Prompt the user for a yes/no confirmation. Returns true for "y" or "yes".
pub fn confirm(prompt: &str) -> bool {
    let prompt_line = format!("{} [y/N]: ", prompt);
    match crate::readline::read_line_with_history(&prompt_line) {
        Ok(Some(line)) => matches!(line.trim().to_lowercase().as_str(), "y" | "yes"),
        _ => false,
    }
}

/// Run the interactive CLI loop using the provided VFS.
/// Blocks until the user exits the shell.
pub fn run_cli(vfs: &mut Vfs, verbose: bool) -> Result<(), String> {
    println!("CLI ready. Type 'help' for commands.");

    // Load filesystem commands
    let mut commands: Vec<Box<dyn ShellCommand>> = register_fs_commands();

    // Build help list
    let mut names: Vec<&'static str> =
        commands.iter().map(|c: &Box<dyn ShellCommand>| c.name()).collect();

    names.push("help");
    names.push("exit");
    names.push("pasta");
    names.push("run");

    // Add built-ins
    commands.push(Box::new(HelpCommand { commands: names.clone() }));
    commands.push(Box::new(ExitCommand));
    commands.push(Box::new(PastaCommand));
    commands.push(Box::new(RunCommand));

    loop {
        // Build prompt showing current working directory
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let prompt = format!("{} $ ", cwd);

        // Use the shared raw-mode line editor (handles arrow keys, history, cursor)
        match crate::readline::read_line_with_history(&prompt) {
            Ok(Some(line)) => {
                // Save into shared history
                crate::readline::history_push(&line);

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // --- Pipeline syntax: left.ps|right.ps  OR  pasta left.ps|right.ps
                //     OR  ./bin args left.ps|right.ps  (binary invocation prefix stripped)
                if trimmed.contains('|') && !trimmed.starts_with(':') {
                    // Split on first '|' to get the raw left and right halves.
                    let (raw_left, raw_right) = {
                        let mut it = trimmed.splitn(2, '|');
                        let l = it.next().unwrap_or("").trim();
                        let r = it.next().unwrap_or("").trim();
                        (l, r)
                    };
                    // From each half, extract the last whitespace-token that ends in ".ps".
                    // This strips any leading binary path (e.g. "./target/release/pasta").
                    fn extract_ps(side: &str) -> Option<&str> {
                        side.split_whitespace().filter(|t| t.ends_with(".ps")).last()
                    }
                    if let (Some(left_script), Some(right_script)) = (extract_ps(raw_left), extract_ps(raw_right)) {
                        let left = if std::path::Path::new(left_script).is_absolute() {
                            left_script.to_string()
                        } else {
                            vfs.local_cwd.join(left_script).to_string_lossy().to_string()
                        };
                        let right = if std::path::Path::new(right_script).is_absolute() {
                            right_script.to_string()
                        } else {
                            vfs.local_cwd.join(right_script).to_string_lossy().to_string()
                        };
                        let left_id  = crate::threading::thread_api::spawn_script_thread(&left,  format!("shell-pipeline-left-{}", left_script));
                        let right_id = crate::threading::thread_api::spawn_script_thread(&right, format!("shell-pipeline-right-{}", right_script));
                        match (left_id, right_id) {
                            (Ok(l), Ok(r)) => println!("spawned pipeline: {} (THID:{}) | {} (THID:{})", left_script, l, right_script, r),
                            (Err(e), _) | (_, Err(e)) => println!("failed to spawn pipeline: {}", e),
                        }
                        continue;
                    }
                }

                let mut parts = trimmed.split_whitespace();
                let cmd_name = parts.next().unwrap();
                let args: Vec<&str> = parts.collect();

                if verbose {
                    println!("[cli] command: {}", cmd_name);
                    println!("[cli] args: {:?}", args);
                }

                // --- New feature: execute binary files or run .ps scripts via pasta binary ---
                // If the user typed a path containing a slash and it points to an executable file,
                // spawn it directly. If the user typed a .ps file path, run it with the current
                // pasta binary (so scripts execute with the interpreter).
                let path = PathBuf::from(cmd_name);
                let looks_like_path = cmd_name.contains('/') || cmd_name.starts_with("./") || cmd_name.starts_with("../");
                let is_ps_file = cmd_name.ends_with(".ps") && path.is_file();
                let is_executable_file = looks_like_path && path.is_file() && is_executable(path.as_path());

                if is_executable_file || is_ps_file {
                    // Ensure terminal is in cooked mode so the child receives canonical input and echo
                    if let Err(e) = ensure_cooked() {
                        eprintln!("warning: failed to restore terminal mode: {}", e);
                    }

                    if is_executable_file {
                        // Restrict execution to safe directories to prevent privilege escalation.
                        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                        let canonical_str = canonical.to_string_lossy();
                        #[cfg(unix)]
                        let allowed = canonical_str.starts_with("/bin/")
                            || canonical_str.starts_with("/usr/bin/")
                            || canonical_str.starts_with("/usr/local/bin/")
                            || canonical_str.starts_with("/usr/sbin/")
                            || canonical_str.starts_with("/sbin/")
                            || canonical_str.starts_with("./")
                            || !canonical_str.starts_with('/'); // relative paths allowed
                        #[cfg(not(unix))]
                        let allowed = true;

                        if !allowed {
                            println!("Permission denied: '{}' is not in an allowed directory", cmd_name);
                            println!("Allowed: /bin, /usr/bin, /usr/local/bin, or relative paths (./...)");
                            continue;
                        }

                        // Spawn the executable the user typed (e.g., ./target/release/pasta)
                        let mut cmd = ProcessCommand::new(&path);
                        for a in &args {
                            cmd.arg(a);
                        }
                        cmd.stdin(Stdio::inherit())
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit());

                        match cmd.spawn() {
                            Ok(mut child) => {
                                if let Err(e) = child.wait() {
                                    println!("Failed to wait for child process: {}", e);
                                }
                            }
                            Err(e) => {
                                println!("Failed to spawn process '{}': {}", cmd_name, e);
                            }
                        }
                    } else {
                        // is_ps_file: run the current pasta binary with the script path and any args
                        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("./target/release/pasta"));
                        let mut cmd = ProcessCommand::new(exe);
                        cmd.arg(cmd_name);
                        for a in &args {
                            cmd.arg(a);
                        }
                        cmd.stdin(Stdio::inherit())
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit());

                        match cmd.spawn() {
                            Ok(mut child) => {
                                if let Err(e) = child.wait() {
                                    println!("Failed to wait for child process: {}", e);
                                }
                            }
                            Err(e) => {
                                println!("Failed to spawn pasta for script '{}': {}", cmd_name, e);
                            }
                        }
                    }

                    // After child exits, continue CLI loop
                    continue;
                }

                // --- Normal internal CLI command handling ---
                if let Some(cmd) =
                    commands.iter().find(|c: &&Box<dyn ShellCommand>| c.name() == cmd_name)
                {
                    match cmd.run(&args, vfs) {
                        Ok(_child) => {
                            // internal command succeeded; nothing special to track here
                        }
                        Err(e) => {
                            if e == "__RETURN_TO_PASTA__" {
                                break;
                            }
                            println!("{}", e);
                        }
                    }
                } else {
                    println!("Invalid command, use 'help' to list available commands.");
                }
            }

            Ok(None) => {
                // EOF / Ctrl-D — exit the CLI loop cleanly
                println!("\nExiting.");
                break;
            }

            Err(e) => {
                println!("Error reading input: {}. Try again.", e);
            }
        }
    }

    Ok(())
}

/// Best-effort: restore terminal to canonical + echo mode so spawned children behave interactively.
fn ensure_cooked() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::mem::zeroed;
        let fd = libc::STDIN_FILENO;
        let mut t: libc::termios = unsafe { zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut t) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        t.c_lflag |= libc::ICANON | libc::ECHO | libc::ISIG;
        t.c_oflag |= libc::OPOST;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &t) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

/// Return true if the given path is executable by the current user (best-effort).
fn is_executable(path: &Path) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode();
            return (mode & 0o111) != 0;
        }
        false
    }
    #[cfg(not(unix))]
    {
        // On non-unix platforms, treat an existing file as executable (best-effort).
        true
    }
}

struct HelpCommand {
    pub commands: Vec<&'static str>,
}

impl ShellCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn run(&self, _args: &[&str], _vfs: &mut Vfs) -> Result<(), String> {
        println!("Available commands:");
        for c in &self.commands {
            println!("  {}", c);
        }
        Ok(())
    }
}

struct ExitCommand;

impl ShellCommand for ExitCommand {
    fn name(&self) -> &'static str {
        "exit"
    }

    fn run(&self, _args: &[&str], _vfs: &mut Vfs) -> Result<(), String> {
        println!("Returning to Pasta interpreter...");
        Err("__RETURN_TO_PASTA__".into())
    }
}

struct PastaCommand;
impl ShellCommand for PastaCommand {
    fn name(&self) -> &'static str { "pasta" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.is_empty() {
            eprintln!("usage: pasta <script.ps>");
            return Ok(());
        }
        let bin = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("pasta"));
        // Resolve script path relative to the VFS local cwd
        let resolved: Vec<String> = args.iter().enumerate().map(|(i, arg)| {
            if i == 0 {
                let p = std::path::Path::new(arg);
                if p.is_absolute() {
                    arg.to_string()
                } else {
                    vfs.local_cwd.join(p).to_string_lossy().to_string()
                }
            } else {
                arg.to_string()
            }
        }).collect();
        match std::process::Command::new(bin).args(&resolved).status() {
            Ok(s) => {
                if !s.success() { eprintln!("pasta exited with status {}", s); }
                Ok(())
            }
            Err(e) => Err(format!("failed to run pasta: {}", e)),
        }

    }
}

/// `run` command - execute PASTA scripts directly
/// Usage: run script.ps [args...]
///        run script1.ps|script2.ps   (pipeline mode)
struct RunCommand;
impl ShellCommand for RunCommand {
    fn name(&self) -> &'static str { "run" }
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String> {
        if args.is_empty() {
            println!("Usage: run <script.ps> [args...]");
            println!("       run <script1.ps>|<script2.ps>   (pipeline mode)");
            return Ok(());
        }
        
        let first_arg = args[0];
        
        // Check for pipeline syntax: run script1.ps|script2.ps
        if first_arg.contains('|') {
            let parts: Vec<&str> = first_arg.split('|').collect();
            if parts.len() == 2 {
                let left_script = parts[0].trim();
                let right_script = parts[1].trim();
                
                // Resolve paths relative to VFS local cwd
                let left = if std::path::Path::new(left_script).is_absolute() {
                    left_script.to_string()
                } else {
                    vfs.local_cwd.join(left_script).to_string_lossy().to_string()
                };
                let right = if std::path::Path::new(right_script).is_absolute() {
                    right_script.to_string()
                } else {
                    vfs.local_cwd.join(right_script).to_string_lossy().to_string()
                };
                
                // Spawn pipeline threads
                let left_id = crate::threading::thread_api::spawn_script_thread(
                    &left, 
                    format!("run-pipeline-left-{}", left_script)
                );
                let right_id = crate::threading::thread_api::spawn_script_thread(
                    &right, 
                    format!("run-pipeline-right-{}", right_script)
                );
                
                match (left_id, right_id) {
                    (Ok(l), Ok(r)) => {
                        println!("Pipeline started: {} (THID:{}) | {} (THID:{})", 
                            left_script, l, right_script, r);
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        return Err(format!("Failed to spawn pipeline: {}", e));
                    }
                }
                return Ok(());
            }
        }
        
        // Single script execution
        let script_path = first_arg;
        let script_args = &args[1..];
        
        // Resolve script path relative to VFS local cwd
        let resolved_path = if std::path::Path::new(script_path).is_absolute() {
            script_path.to_string()
        } else {
            vfs.local_cwd.join(script_path).to_string_lossy().to_string()
        };
        
        // Check if file exists
        if !std::path::Path::new(&resolved_path).exists() {
            return Err(format!("Script not found: {}", resolved_path));
        }
        
        // Get the pasta binary
        let bin = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("pasta"));
        
        // Build command: pasta <script> [args...]
        let mut cmd = std::process::Command::new(bin);
        cmd.arg(&resolved_path);
        for arg in script_args {
            cmd.arg(arg);
        }
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        
        match cmd.spawn() {
            Ok(mut child) => {
                if let Err(e) = child.wait() {
                    return Err(format!("Failed to wait for script: {}", e));
                }
                Ok(())
            }
            Err(e) => Err(format!("Failed to run script '{}': {}", script_path, e)),
        }
    }
}


