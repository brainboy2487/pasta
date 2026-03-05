// agent.rs - minimal agent skeleton (JSON-over-stdio)
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct Handshake {{
    agent_version: String,
    capabilities: Vec<String>,
}}

fn main() {{
    // Simple handshake
    let hs = Handshake {{
        agent_version: "0.1.0".into(),
        capabilities: vec!["exec".into(), "logs".into()],
    }};
    println!("HANDSHAKE:{{}}", serde_json::to_string(&hs).unwrap());
    io::stdout().flush().unwrap();

    // Heartbeat loop (for demo)
    loop {{
        println!("HEARTBEAT");
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(5));
        // In a real agent, read commands from stdin and act on them.
    }}
}}
