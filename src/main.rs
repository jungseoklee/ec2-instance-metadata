use chrono::Utc;
use clap::{Parser, Subcommand};
use std::{
    error::Error, sync::{Arc, RwLock}, thread, time::{Duration, Instant},
};

const ENDPOINT: &str = "http://169.254.169.254";
const TOKEN_TTL: Duration = Duration::from_hours(6);
const TOKEN_REFRESH_OFFSET: Duration = Duration::from_hours(1);

#[derive(Parser)]
#[command(name = "ec2im")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Get {
        path: String,
    },
    Poll {
        path: String,
        #[arg(long, default_value_t = 5000, value_parser = clap::value_parser!(u64).range(100..=5000))]
        interval: u64,
    },
}

fn get_token() -> Result<String, Box<dyn Error>> {
    let output = std::process::Command::new("curl")
        .arg("--max-time")
        .arg("2")
        .arg("-X")
        .arg("PUT")
        .arg(format!("{}/latest/api/token", ENDPOINT))
        .arg("-H")
        .arg(format!("X-aws-ec2-metadata-token-ttl-seconds: {}", TOKEN_TTL.as_secs()))
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err("Failed to get token.".into())
    }
}

fn query(token: &str, path: &str) -> Result<String, Box<dyn Error>> {
    let url = format!("{}/latest/{}", ENDPOINT, path);
    let output = std::process::Command::new("curl")
        .arg("-H")
        .arg(format!("X-aws-ec2-metadata-token: {}", token))
        .arg(&url)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err("Failed to get data for {path}.".into())
    }
}

fn poll(init_token: String, path: &str, interval_ms: u64) -> Result<(), Box<dyn Error>> {
    let interval = Duration::from_millis(interval_ms);
    let token = Arc::new(RwLock::new(init_token));
    let token_obtained_at = Arc::new(RwLock::new(Instant::now()));

    let token_clone = Arc::clone(&token);
    let token_obtained_at_clone = Arc::clone(&token_obtained_at);

    thread::spawn(move || {
        loop {
            let refresh_interval = TOKEN_TTL - TOKEN_REFRESH_OFFSET;
            let sleep_until = *token_obtained_at_clone.read().unwrap() + refresh_interval;
            let now = Instant::now();
            if sleep_until > now {
                thread::sleep(sleep_until - now);
            }
            loop {
                if let Ok(new_token) = get_token() {
                    *token_clone.write().unwrap() = new_token;
                    *token_obtained_at_clone.write().unwrap() = Instant::now();
                    break;
                }
                thread::sleep(Duration::from_mins(1));
            }
        }
    });

    loop {
        let current_token = token.read().unwrap().clone();
        match query(&current_token, path) {
            Ok(res) => println!("{}: {res}", Utc::now()),
            Err(e) => eprintln!("{}: {e}", Utc::now()),
        }
        thread::sleep(interval);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let token = get_token()?;

    match cli.command {
        Command::Get { path } => {
            match query(&token, &path) {
                Ok(res) => println!("{}: {res}", Utc::now()),
                Err(e) => eprintln!("{}: {e}", Utc::now()),
            }
        },
        Command::Poll { path, interval } => {
            poll(token, &path, interval)?;
        }
    }

    Ok(())
}
