mod chunk_manager;
mod commands;
mod net;
mod persistence;
mod player;
mod server;
mod world;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use server::ServerConfig;

fn main() {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();

    let mut world_path = PathBuf::from("world");
    let mut port: u16 = 25565;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--world" => {
                let Some(value) = args.next() else {
                    eprintln!("--world expects a path argument");
                    std::process::exit(2);
                };
                world_path = PathBuf::from(value);
            }
            "--port" => {
                let Some(value) = args.next() else {
                    eprintln!("--port expects a numeric argument");
                    std::process::exit(2);
                };
                match value.parse::<u16>() {
                    Ok(parsed) => port = parsed,
                    Err(err) => {
                        eprintln!("invalid port '{value}': {err}");
                        std::process::exit(2);
                    }
                }
            }
            "--help" | "-h" => {
                println!("Usage: veldspar_server [--world <path>] [--port <u16>]");
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\nShutdown signal received, saving world...");
        r.store(false, Ordering::SeqCst);
    }).expect("failed to set Ctrl+C handler");

    let config = ServerConfig { world_path, port };
    if let Err(err) = server::run(config, running) {
        eprintln!("server failed: {err}");
        std::process::exit(1);
    }
}
