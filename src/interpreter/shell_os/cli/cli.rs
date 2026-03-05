use std::io::{self, Write};

use crate::interpreter::shell_os::commands::{Command, register_fs_commands};
use crate::interpreter::shell_os::vfs::Vfs;

/// Prompt the user for a yes/no confirmation. Returns true for "y" or "yes".
pub fn confirm(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    std::io::stdout().flush().ok();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Run the interactive CLI loop using the provided VFS.
/// Blocks until the user exits the shell.
pub fn run_cli(vfs: &mut Vfs, verbose: bool) -> Result<(), String> {
    println!("CLI ready. Type 'help' for commands.");

    // Load filesystem commands
    let mut commands: Vec<Box<dyn Command>> = register_fs_commands();

    // Build help list
    let mut names: Vec<&'static str> =
        commands.iter().map(|c: &Box<dyn Command>| c.name()).collect();

    names.push("help");
    names.push("exit");

    // Add built-ins
    commands.push(Box::new(HelpCommand { commands: names.clone() }));
    commands.push(Box::new(ExitCommand));

    loop {
        print!("> ");
        io::stdout().flush().ok();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                println!("\nExiting.");
                break;
            }
            Ok(_) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let mut parts = trimmed.split_whitespace();
                let cmd_name = parts.next().unwrap();
                let args: Vec<&str> = parts.collect();

                if verbose {
                    println!("[cli] command: {}", cmd_name);
                    println!("[cli] args: {:?}", args);
                }

                if let Some(cmd) =
                    commands.iter().find(|c: &&Box<dyn Command>| c.name() == cmd_name)
                {
                    match cmd.run(&args, vfs) {
                        Ok(_) => {}
                        Err(e) => println!("{}", e),
                    }
                } else {
                    println!("Invalid command, use 'help' to list available commands.");
                }
            }
            Err(_) => {
                println!("Error reading input. Try again.");
            }
        }
    }

    Ok(())
}

struct HelpCommand {
    pub commands: Vec<&'static str>,
}

impl Command for HelpCommand {
    fn name(&self) -> &'static str { "help" }

    fn run(&self, _args: &[&str], _vfs: &mut Vfs) -> Result<(), String> {
        println!("Available commands:");
        for c in &self.commands {
            println!("  {}", c);
        }
        Ok(())
    }
}

struct ExitCommand;

impl Command for ExitCommand {
    fn name(&self) -> &'static str { "exit" }

    fn run(&self, _args: &[&str], _vfs: &mut Vfs) -> Result<(), String> {
        println!("Goodbye.");
        std::process::exit(0);
    }
}
