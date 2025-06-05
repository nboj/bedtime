use std::{
    fs::{self},
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};

use chrono::{Local, NaiveTime};
use clap::Command;
use sled::IVec;

const SOCKET_PATH: &str = "/tmp/bedtime.sock";
const STATUS_PATH: &str = "/tmp/bedtime";

const BED_TIME: NaiveTime =
    NaiveTime::from_hms_opt(22, 0, 0).expect("Invalid target time configuration.");
const WAKEUP_TIME: NaiveTime =
    NaiveTime::from_hms_opt(5, 0, 0).expect("Invalid target time configuration.");

// returns true when ending daemon
fn handle_input(listener: &UnixListener) -> bool {
    let accept = listener.accept();
    match accept {
        Ok((mut stream, _addr)) => {
            println!("Found Connection.");
            let mut reader = BufReader::new(stream.try_clone().expect("Could not clone stream."));
            let mut line = String::new();
            if let Ok(_) = reader.read_line(&mut line) {
                let command = line.trim();
                match command {
                    "status" => {
                        print_status(&mut stream);
                    }
                    "stop" => {
                        let _ = stream.write_all(format!("Ending daemon...").as_bytes());
                        let _ = stream.flush();
                        return true;
                    }
                    _ => {
                        let _ = stream.write_all(format!("Error: Invalid command.").as_bytes());
                        let _ = stream.flush();
                    }
                }
            }
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => {}
        Err(e) => {
            eprintln!("{}", e);
        }
    }
    false
}

fn print_status(stream: &mut UnixStream) {
    let now = Local::now().time();
    let diff = BED_TIME - now;
    if now < BED_TIME && now > WAKEUP_TIME {
        let _ = stream.write_all(
            format!(
                "\nStatus: AWAKE\nTime Left: {:02}:{:02}:{:02}.{:02}\n",
                diff.num_hours(),
                diff.num_minutes() % 60,
                diff.num_seconds() % 60,
                (diff.num_milliseconds() % 1000) / 10,
            )
            .as_bytes(),
        );
        let _ = stream.flush();
    } else {
        let _ = stream.write_all(format!("\nStatus: ASLEEP\nTime for bed.").as_bytes());
        let _ = stream.flush();
    }
}

fn run_daemon() {
    println!("Enabling...");
    if Path::new(SOCKET_PATH).exists() {
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket.");
    let _ = listener.set_nonblocking(true);
    println!("Enjoy!");

    let tree = sled::open(STATUS_PATH).expect("Error: could not open sled db");

    loop {
        let now = Local::now().time();
        let triggered = match tree.get("triggered").expect("Could not get triggered") {
            Some(v) => v,
            None => IVec::from("false"),
        };
        if now >= BED_TIME || now <= WAKEUP_TIME {
            if triggered == IVec::from("false") {
                println!("triggerringngngngngn");
                let _ = tree.insert("triggered", "true");
                let _ = tree.flush();
            }
        } else if triggered == IVec::from("true") {
            let _ = tree.insert("triggered", "false");
            let _ = tree.flush();
        }
        let should_end = handle_input(&listener);
        if should_end {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let _ = std::fs::remove_file(SOCKET_PATH);
    println!("Daemon stopped.");
}

fn send_command(cmd: &str) {
    if !Path::new(SOCKET_PATH).exists() {
        eprintln!("Error: Daemon is not running.");
        return;
    }
    match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => {
            let _ = stream.write_all(format!("{}\n", cmd).as_bytes());
            let _ = stream.flush();
            let mut reader = BufReader::new(stream);
            let mut response = String::new();
            let _ = reader.read_to_string(&mut response);
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon {}", e);
        }
    }
}

fn main() {
    let matches = Command::new("bedtime")
        .subcommand(Command::new("start").about("Start program"))
        .subcommand(Command::new("status").about("Check program status"))
        .subcommand(Command::new("stop").about("Stops the program"))
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
            run_daemon();
        }
        Some(("status", _matches)) => {
            send_command("status");
        }
        Some(("stop", _matches)) => {
            send_command("stop");
        }
        Some(_) | None => {
            eprintln!("Not a command.");
            return;
        }
    }
}
