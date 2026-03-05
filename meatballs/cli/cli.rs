// cli.rs - simple CLI stub for Saucepan -> Meatball interactions
use crate::api::types::Resources;
use crate::api::LocalBackend;

pub fn spawn_example() {{
    let backend = LocalBackend;
    let res = Resources {{ memory_mib: 128, vcpus: 1, disk_mib: 64, network: false }};
    match backend.spawn(res, None, None) {{
        Ok(id) => println!("spawned meatball {}", id),
        Err(e) => eprintln!("spawn failed: {}", e),
    }}
}}
