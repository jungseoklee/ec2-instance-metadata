use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    error::Error, sync::{Arc, OnceLock, RwLock}, thread, time::{Duration, Instant},
};

const ENDPOINT: &str = "http://169.254.169.254";
const TOKEN_TTL: Duration = Duration::from_hours(6);
const TOKEN_REFRESH_OFFSET: Duration = Duration::from_hours(2);

static TIMESTAMP_FORMAT: OnceLock<TimestampFormat> = OnceLock::new();

#[derive(Parser)]
#[command(name = "ec2im", about = "EC2 Instance Metadata CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, short = 't', value_enum, default_value_t = TimestampFormat::Iso, global = true)]
    timestamp_format: TimestampFormat,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Get instance metadata once")]
    Get {
        path: String,
    },
    #[command(about = "Get instance metadata periodically")]
    Poll {
        path: String,
        #[arg(long, short = 'i', default_value_t = 5000, value_parser = clap::value_parser!(u64).range(500..=10000))]
        interval: u64,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum TimestampFormat {
    Iso,
    Unix,
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
        Err(format!("Failed to get data for {path}.").into())
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
        println!("{}", to_json(&path, query(&current_token, path)));
        thread::sleep(interval);
    }
}

fn to_json(path: &str, res: Result<String, Box<dyn Error>>) -> String {
    let timestamp = match TIMESTAMP_FORMAT.get().unwrap_or(&TimestampFormat::Iso) {
        TimestampFormat::Iso => Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
        TimestampFormat::Unix =>  Utc::now().timestamp_millis().to_string()
    };
    match res {
        Ok(v) => format!(
            r#"{{"timestamp": "{}", "path": "{}", "value": "{}", "status": "success"}}"#,
            timestamp, path, v),
        Err(e) => format!(
            r#"{{"timestamp": "{}", "path": "{}", "value": null, "status": "error", "reason": "{}"}}"#,
            timestamp, path, e),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    TIMESTAMP_FORMAT.set(cli.timestamp_format).unwrap();

    let token = get_token()?;
    match cli.command {
        Command::Get { path } => {
            println!("{}", to_json(&path, query(&token, &path)))
        },
        Command::Poll { path, interval } => {
            poll(token, &path, interval)?;
        }
    }

    Ok(())
}
