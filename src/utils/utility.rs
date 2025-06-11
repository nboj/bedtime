use std::{
    io::{BufRead, BufReader, ErrorKind, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::{Arc, Mutex},
};

use chrono::{Local, NaiveTime};
use sled::{Db, IVec};

use crate::utils::spotify::run_spotify;

const BED_TIME: NaiveTime =
    NaiveTime::from_hms_opt(22, 0, 0).expect("Invalid target time configuration.");
const WAKEUP_TIME: NaiveTime =
    NaiveTime::from_hms_opt(5, 0, 0).expect("Invalid target time configuration.");
const SOCKET_PATH: &str = "/tmp/bedtime.sock";
const STATUS_PATH: &str = "/tmp/bedtime";

// returns true when ending daemon
fn handle_input(listener: &UnixListener, tree: &Db) -> bool {
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
                    "reset" => {
                        let _ = tree.insert("triggered", "false");
                        let _ = stream.write_all(format!("Reset variables.").as_bytes());
                        let _ = stream.flush();
                    }
                    "test" => {}
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

async fn trigger_bedtime(tree: &Db) {
    println!("Triggering events...");
    let _ = tree.insert("triggered", "true");
    let _ = tree.flush();
    run_spotify().await;
    // NOTE: needs to allow shutdown without sudo
    let command = std::process::Command::new("sh")
        .arg("-c")
        .arg("ssh cauman@192.168.1.62 \"sudo shutdown now\"")
        //.arg("shutdown now")
        .output()
        .expect("Failed to execute process");

    let stdout = String::from_utf8_lossy(&command.stdout);
    let stderr = String::from_utf8_lossy(&command.stderr);

    println!("stdout: {}", stdout);
    if !stderr.is_empty() {
        eprintln!("stderr: {}", stderr);
    }
}

pub async fn run_daemon() {
    println!("Enabling...");
    let mut triggered_warning = false;
    if Path::new(SOCKET_PATH).exists() {
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket.");
    //let _ = listener.set_nonblocking(true);
    println!("Enjoy!");
    let should_end = Arc::new(Mutex::new(false));

    let tree = sled::open(STATUS_PATH).expect("Error: could not open sled db");

    let _tree = tree.clone();
    let _end = should_end.clone();
    let thread = std::thread::spawn(move || {
        loop {
            let _should_end = handle_input(&listener, &_tree);
            if _should_end {
                let mut end = _end.lock().expect("Cannot lock should_end...");
                *end = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    let _end = should_end.clone();
    loop {
        let now = Local::now().time();
        let triggered = match tree.get("triggered").expect("Could not get triggered") {
            Some(v) => v,
            None => IVec::from("false"),
        };
        if now >= BED_TIME || now <= WAKEUP_TIME {
            if triggered == IVec::from("false") {
                trigger_bedtime(&tree).await;
            }
        } else if triggered == IVec::from("true") {
            let _ = tree.insert("triggered", "false");
            let _ = tree.flush();
        }
        if !triggered_warning && (BED_TIME - now).num_seconds() < 20 * 60 {
            println!("Warned shut down in 5 minutes.");
            triggered_warning = true;
            let _command = std::process::Command::new("sh")
                .arg("-c")
                .arg("ssh cauman@192.168.1.62 \"notify-send 'Shutting down in 5 minutes...'\"")
                //.arg("shutdown now")
                .output()
                .expect("Failed to execute process");
        }
        {
            if *_end.lock().expect("Could not lock should_end") {
                break;
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let _ = std::fs::remove_file(SOCKET_PATH);
    println!("Daemon stopped.");
    thread.join().unwrap();
}
