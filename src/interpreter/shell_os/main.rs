mod cli;
mod commands;
mod vfs;

use std::env;
use vfs::Vfs;

/// Global runtime configuration.
struct RuntimeConfig {
    pub verbose: bool,
}

impl RuntimeConfig {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");

        RuntimeConfig { verbose }
    }
}

mod ops_log;


fn main() {
    let config = RuntimeConfig::from_args();

    if config.verbose {
        println!("[runtime] Verbose logging enabled");
        println!("[runtime] Booting virtual filesystem...");
    }

    // Boot the VFS
    let mut vfs = Vfs::boot();

    if config.verbose {
        println!("[runtime] Filesystem mounted");
        println!("[runtime] Current working directory: /");
    }

    // Launch CLI loop
    if let Err(e) = cli::run_cli(&mut vfs, config.verbose) {
        eprintln!("[fatal] {}", e);
    }
}
