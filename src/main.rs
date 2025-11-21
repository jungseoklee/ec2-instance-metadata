use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::{
    error::Error, sync::{Arc, RwLock}, thread, time::{Duration, Instant},
};

const ENDPOINT: &str = "http://169.254.169.254";
const TOKEN_TTL: Duration = Duration::from_secs(21600);
const TOKEN_REFRESH_OFFSET: Duration = Duration::from_secs(10800);

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

struct GlobalConfig {
    timestamp_format: TimestampFormat,
}

impl GlobalConfig {
    fn new(timestamp_format: TimestampFormat) -> Self {
        Self { timestamp_format }
    }
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

fn poll(init_token: String, path: &str, interval_ms: u64, config: &GlobalConfig) -> Result<(), Box<dyn Error>> {
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
                thread::sleep(Duration::from_secs(60));
            }
        }
    });

    loop {
        let current_token = token.read().unwrap().clone();
        println!("{}", to_json(query(&current_token, path), &config));
        thread::sleep(interval);
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Timestamp {
    Iso(String),
    Unix(i64),
}

#[derive(Serialize)]
struct Output {
    timestamp: Timestamp,
    #[serde(flatten)]
    result: QueryResult,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum QueryResult {
    Success {
        value: String,
    },
    Error {
        value: Option<String>,
        reason: String,
    }
}

fn to_json(res: Result<String, Box<dyn Error>>, config: &GlobalConfig) -> String {
    let timestamp = match config.timestamp_format {
        TimestampFormat::Iso => Timestamp::Iso(
            Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
        ),
        TimestampFormat::Unix => Timestamp::Unix(Utc::now().timestamp_millis()),
    };
    let query_result = match res {
        Ok(v) => QueryResult::Success { value: v },
        Err(e) => QueryResult::Error { value: None, reason: e.to_string() },
    };
    let output = Output {
        timestamp,
        result: query_result,
    };

    serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let config = GlobalConfig::new(cli.timestamp_format);
    let token = get_token()?;

    match cli.command {
        Command::Get { path } => {
            println!("Querying {path}...");
            println!("{}", to_json(query(&token, &path), &config));
        },
        Command::Poll { path, interval } => {
            println!("Polling {path}...");
            poll(token, &path, interval, &config)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_to_json_success_iso_timestamp() {
        // given
        let result = Ok("i-0b22a22eec53b9321".to_string());
        let config = GlobalConfig::new(TimestampFormat::Iso);

        // when
        let res = to_json(result, &config);

        // then
        let ser_res: Value = serde_json::from_str(&res).expect("valid json");
        assert_eq!(ser_res["status"], "success");
        assert_eq!(ser_res["value"], "i-0b22a22eec53b9321");
        assert!(ser_res["timestamp"].is_string());
    }

    #[test]
    fn test_to_json_success_unix_timestamp() {
        // given
        let result = Ok("i-0b22a22eec53b9321".to_string());
        let config = GlobalConfig::new(TimestampFormat::Unix);

        // when
        let res = to_json(result, &config);

        // then
        let ser_res: Value = serde_json::from_str(&res).expect("valid json");
        assert_eq!(ser_res["status"], "success");
        assert_eq!(ser_res["value"], "i-0b22a22eec53b9321");
        assert!(ser_res["timestamp"].is_number());
    }

    #[test]
    fn test_to_json_error() {
        // given
        let result = Err("connection timeout".into());
        let config = GlobalConfig::new(TimestampFormat::Unix);

        // when
        let res = to_json(result, &config);

        // then
        let ser_res: Value = serde_json::from_str(&res).expect("valid json");
        assert_eq!(ser_res["value"], Value::Null);
    }
}
