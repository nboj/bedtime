use std::{
    fs::{self},
    io::{BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
};

use clap::Command;

use crate::utils::{spotify::run_spotify, utility::run_daemon};

pub mod utils;

const SOCKET_PATH: &str = "/tmp/bedtime.sock";

fn send_command(cmd: &str) {
    if !Path::new(SOCKET_PATH).exists() {
        eprintln!("Error: Daemon is not running.");
        return;
    }
    match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => {
            let _ = stream.write_all(format!("{}\n", cmd).as_bytes());
            let _ = stream.flush();
            let mut reader = BufReader::new(&stream);
            let mut response = String::new();
            println!("HERE");
            let _ = reader.read_to_string(&mut response);
            println!("NOW HERE");
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon {}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let matches = Command::new("bedtime")
        .subcommand(Command::new("start").about("Start program"))
        .subcommand(Command::new("status").about("Check program status"))
        .subcommand(Command::new("stop").about("Stops the program"))
        .subcommand(Command::new("reset").about("Resets the program"))
        .subcommand(Command::new("spotify").about("Tests spotify api"))
        .subcommand(Command::new("test").about("Tests the entire shutdown process"))
        .get_matches();
    match matches.subcommand() {
        Some(("start", _matches)) => {
            if Path::new(SOCKET_PATH).exists() {
                match UnixStream::connect(SOCKET_PATH) {
                    Err(e) => {
                        eprintln!("Failed to connect to daemon {}\nResetting socket.", e);
                        let _ = fs::remove_file(SOCKET_PATH);
                    }
                    _ => {
                        eprintln!("Daemon already running.");
                        return;
                    }
                }
            }
            run_daemon().await;
        }
        Some(("status", _matches)) => {
            send_command("status");
        }
        Some(("stop", _matches)) => {
            send_command("stop");
        }
        Some(("reset", _matches)) => {
            send_command("reset");
        }
        Some(("spotify", _matches)) => {
            run_spotify().await;
        }
        Some(("test", _matches)) => {
            send_command("test");
        }
        Some(_) | None => {
            eprintln!("Not a command.");
            return;
        }
    }
}
