//! Conversion Replay Tool
//!
//! Replays conversion samples from telemetry to test field mapping changes.
//! Usage:
//!   convert-replay --file path/to/captured.json

use bytes::Bytes;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use zz::converter::{ApiType, AnthropicToOpenAIConverter, OpenAIChatToAnthropicConverter, ApiConverter};

#[derive(Parser, Debug)]
#[command(name = "convert-replay")]
#[command(about = "Replay conversion samples from telemetry", long_about = None)]
struct Args {
    /// Sample ID from telemetry
    #[arg(long)]
    sample_id: Option<u64>,

    /// Event signature hash
    #[arg(long)]
    signature: Option<String>,

    /// Path to JSON file containing request/response
    #[arg(long)]
    file: Option<PathBuf>,

    /// Direction: request or response
    #[arg(long, default_value = "request")]
    direction: String,

    /// Source API type
    #[arg(long, default_value = "Anthropic")]
    source: String,

    /// Target API type
    #[arg(long, default_value = "OpenAIChat")]
    target: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SampleFile {
    request_body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_body: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load sample data
    let (request_body, response_body) = if let Some(file_path) = args.file {
        let content = fs::read_to_string(&file_path)?;
        let sample: SampleFile = serde_json::from_str(&content)?;
        (Bytes::from(sample.request_body), sample.response_body.map(Bytes::from))
    } else {
        eprintln!("Error: must provide either --file, --sample-id, or --signature");
        eprintln!("Note: --sample-id and --signature require connecting to running ZZ server (not implemented yet)");
        std::process::exit(1);
    };

    let source = ApiType::from_str(&args.source)
        .map_err(|e| format!("Invalid source API type: {}", e))?;
    let target = ApiType::from_str(&args.target)
        .map_err(|e| format!("Invalid target API type: {}", e))?;

    println!("=== Conversion Replay ===");
    println!("Direction: {}", args.direction);
    println!("Source: {:?}", source);
    println!("Target: {:?}", target);
    println!();

    if args.direction == "request" {
        let converter = AnthropicToOpenAIConverter;
        match converter.convert_request(&request_body, target) {
            Ok(result) => {
                println!("=== Conversion Result ===");
                println!("{}", String::from_utf8_lossy(&result));
            }
            Err(e) => {
                eprintln!("=== Conversion Error ===");
                eprintln!("{}", e);
                eprintln!("Code: {}", e.code);
                if let Some(path) = &e.field_path {
                    eprintln!("Field path: {}", path);
                }
            }
        }
    } else if args.direction == "response" {
        let converter = OpenAIChatToAnthropicConverter;
        let body = response_body.as_ref().ok_or("Response body required for direction=response")?;
        match converter.convert_response(body, source, target, false) {
            Ok(result) => {
                println!("=== Conversion Result ===");
                println!("{}", String::from_utf8_lossy(&result));
            }
            Err(e) => {
                eprintln!("=== Conversion Error ===");
                eprintln!("{}", e);
                eprintln!("Code: {}", e.code);
                if let Some(path) = &e.field_path {
                    eprintln!("Field path: {}", path);
                }
            }
        }
    } else {
        eprintln!("Error: direction must be 'request' or 'response'");
        std::process::exit(1);
    }

    Ok(())
}
