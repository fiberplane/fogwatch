// Main entry point for the fogwatch application.
// Provides real-time monitoring of Cloudflare Workers logs with filtering capabilities
// and multiple output formats.

mod config;
mod tui;
mod types;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use config::Config;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use reqwest;
use std::fs;
use std::io;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tui::{run_ui, App};
use crate::types::*;

// Command-line argument structure for fogwatch
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "A real-time log viewer for Cloudflare Workers with filtering and multiple output formats",
    long_about = "Monitor your Cloudflare Workers logs in real-time with advanced filtering, multiple output formats, and interactive worker selection."
)]
struct Args {
    /// Your Cloudflare API token (can also be set via CF_API_TOKEN env var)
    #[arg(short, long)]
    token: Option<String>,

    /// Account ID (can also be set via CF_ACCOUNT_ID env var)
    #[arg(short, long)]
    account_id: Option<String>,

    /// Worker name (can also be set via CF_WORKER_NAME env var)
    #[arg(short, long)]
    worker: Option<String>,

    /// Filter by HTTP method (e.g., GET, POST)
    #[arg(short = 'm', long)]
    method: Option<Vec<String>>,

    /// Search string in logs
    #[arg(short = 'q', long)]
    search: Option<String>,

    /// Sampling rate (0.0 to 1.0)
    #[arg(short = 'r', long)]
    sampling_rate: Option<f64>,

    /// Filter by status (ok, error, canceled)
    #[arg(short = 's', long)]
    status: Option<Vec<String>>,

    /// Enable debug output
    #[arg(short = 'd', long)]
    debug: bool,

    /// Output format for logs:
    ///   - pretty: Colored, human-readable format (default)
    ///   - json: Raw JSON format for machine processing
    ///   - compact: Single-line format for grep/awk
    #[arg(short = 'f', long, default_value = "pretty")]
    format: OutputFormat,

    /// Generate a sample configuration file (fogwatch.toml.sample)
    #[arg(long = "init-config")]
    init_config: bool,
}

/// Creates a new tail session with Cloudflare's API
///
/// # Arguments
/// * `client` - HTTP client for making API requests
/// * `args` - Command line arguments containing API credentials and filters
/// * `filters` - Collection of filters to apply to the log stream
/// * `tx` - Channel for sending log events back to the main thread
///
/// # Returns
/// * `Result<TailCreationResponse>` - WebSocket URL and session details for the tail
async fn create_tail(
    client: &reqwest::Client,
    args: &Args,
    filters: Vec<TailFilter>,
    tx: &mpsc::Sender<TailEventMessage>,
) -> Result<TailCreationResponse> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts/{}/tails",
        args.account_id.as_ref().unwrap(),
        args.worker.as_ref().unwrap()
    );

    if args.debug {
        let log = create_log_entry(
            format!(
                "Creating tail session for worker '{}'...",
                args.worker.as_ref().unwrap()
            ),
            "debug",
        );
        tx.send(create_tail_event(log)).ok();
    }

    let response = client
        .post(&url)
        .json(&TailRequest { filters })
        .header("Accept", "application/json")
        .send()
        .await?;

    if args.debug {
        let log = create_log_entry(
            format!("HTTP Response: {} for POST {url}", response.status()),
            "debug",
        );
        tx.send(create_tail_event(log)).ok();
    }

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        if args.debug {
            let log = create_log_entry(format!("Error response body: {error_text}"), "debug");
            tx.send(create_tail_event(log)).ok();
        }
        return Err(anyhow!("HTTP error: {status} {error_text}"));
    }

    let text = response.text().await?;
    let data: CloudflareResponse<TailCreationResponse> = serde_json::from_str(&text)?;

    Ok(data.result)
}

/// Converts command-line arguments into Cloudflare API filter format
///
/// Processes the following filter types:
/// - Sampling rate (0.0 to 1.0)
/// - HTTP methods (GET, POST, etc)
/// - Search queries (text to match in logs)
/// - Status filters (ok, error, canceled)
///
/// # Arguments
/// * `args` - Command line arguments containing filter specifications
///
/// # Returns
/// * `Vec<TailFilter>` - Collection of filters in Cloudflare API format
fn parse_filters(args: &Args) -> Vec<TailFilter> {
    let mut filters = vec![
        // Always set sampling rate to 1.0 to get all events
        TailFilter::SamplingRate { sampling_rate: 1.0 },
    ];

    // Add method filter if specified
    if let Some(methods) = &args.method {
        filters.push(TailFilter::Method {
            method: methods.clone(),
        });
    } else {
        // If no methods specified, include all methods
        filters.push(TailFilter::Method {
            method: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
                "HEAD".to_string(),
                "PATCH".to_string(),
                "TRACE".to_string(),
            ],
        });
    }

    // Add search filter if specified
    if let Some(search) = &args.search {
        filters.push(TailFilter::Query {
            query: search.clone(),
        });
    }

    // Add status filter if specified
    match &args.status {
        Some(statuses) => {
            let outcomes: Vec<String> = statuses
                .iter()
                .map(|s| match s.as_str() {
                    "ok" => "ok".to_string(),
                    "error" => "exception".to_string(),
                    "canceled" => "canceled".to_string(),
                    _ => "unknown".to_string(),
                })
                .collect();
            filters.push(TailFilter::Outcome { outcome: outcomes });
        }
        None => {
            // Include all outcomes by default
            filters.push(TailFilter::Outcome {
                outcome: vec![
                    "ok".to_string(),
                    "exception".to_string(),
                    "canceled".to_string(),
                    "exceededCpu".to_string(),
                    "exceededMemory".to_string(),
                ],
            });
        }
    }

    filters
}

/// Creates a new log entry with the specified message and level
///
/// # Arguments
/// * `message` - Log message content
/// * `level` - Severity level of the log entry
///
/// # Returns
/// * `LogEntry` - Formatted log entry ready for display
fn create_log_entry(message: String, level: &str) -> LogEntry {
    LogEntry {
        message: vec![serde_json::Value::String(message)],
        level: level.to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time cannot be behind unix epoch")
            .as_secs() as i64,
    }
}

/// Wraps a log entry in a TailEventMessage for transmission
///
/// # Arguments
/// * `log_entry` - Log entry to wrap
///
/// # Returns
/// * `TailEventMessage` - Message ready for sending through channels
fn create_tail_event(log_entry: LogEntry) -> TailEventMessage {
    let timestamp = log_entry.timestamp;
    TailEventMessage {
        outcome: Outcome::Ok,
        script_name: None,
        exceptions: vec![],
        logs: vec![log_entry],
        event_timestamp: timestamp,
        event: None,
        status: None,
    }
}

/// Retrieves a list of available workers from Cloudflare
///
/// # Arguments
/// * `client` - HTTP client for making API requests
/// * `account_id` - Cloudflare account identifier
/// * `debug` - Whether to output debug information
/// * `tx` - Channel for sending status messages
///
/// # Returns
/// * `Result<Vec<WorkerScript>>` - List of available worker scripts
async fn fetch_workers(
    client: &reqwest::Client,
    account_id: &str,
    debug: bool,
    tx: &mpsc::Sender<TailEventMessage>,
) -> Result<Vec<WorkerScript>> {
    let url = format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/workers/scripts");

    let response = match client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(resp) => {
            if debug {
                let log = create_log_entry(
                    format!("HTTP Response: {} for GET {url}", resp.status()),
                    "debug",
                );
                tx.send(create_tail_event(log)).ok();
            }
            resp
        }
        Err(e) => {
            if debug {
                let log = create_log_entry(format!("Error details: {e}"), "debug");
                tx.send(create_tail_event(log)).ok();
            }
            if e.is_connect() {
                return Err(anyhow!(
                    "Could not connect to Cloudflare API. Please check your internet connection."
                ));
            } else if e.is_timeout() {
                return Err(anyhow!(
                    "Connection to Cloudflare API timed out. Please try again."
                ));
            } else {
                return Err(anyhow!(
                    "Network error: {e}. Please check your connection and try again."
                ));
            }
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        if debug {
            let log = create_log_entry(format!("Error response body: {error_text}"), "debug");
            tx.send(create_tail_event(log)).ok();
        }
        return match status.as_u16() {
            401 => Err(anyhow!(
                "Authentication failed. Please check your API token."
            )),
            403 => Err(anyhow!(
                "Access denied. Please verify your account ID and API token permissions."
            )),
            404 => Err(anyhow!("Account not found. Please verify your account ID.")),
            429 => Err(anyhow!("Rate limit exceeded. Please try again later.")),
            _ => Err(anyhow!("HTTP error {status}: {error_text}")),
        };
    }

    let response: WorkerScriptResponse = match response.json().await {
        Ok(resp) => resp,
        Err(e) => {
            if debug {
                let log = create_log_entry(format!("Error details: {e}"), "debug");
                tx.send(create_tail_event(log)).ok();
            }
            return Err(anyhow!("Failed to parse Cloudflare API response. This might be a temporary issue, please try again."));
        }
    };

    if !response.success {
        let error_msg = response
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.code, e.message))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!("Cloudflare API error: {error_msg}"));
    }

    if response.result.is_empty() {
        return Err(anyhow!("No workers found in your account. Please make sure you have deployed at least one worker."));
    }

    Ok(response.result)
}

/// Main entry point for the application
///
/// Handles:
/// - Command line argument parsing
/// - Configuration loading and validation
/// - API client initialization
/// - Worker selection (interactive or command line)
/// - Log streaming and display
///
/// # Returns
/// * `Result<()>` - Success or error status of the application
fn main() -> Result<()> {
    // Parse command line arguments first
    let args = Args::parse();

    // Handle config initialization if requested
    if args.init_config {
        println!("Generating sample configuration file: fogwatch.toml.sample");
        let sample_config = include_str!("../fogwatch.toml.sample");
        fs::write("fogwatch.toml.sample", sample_config)
            .context("Failed to write sample configuration file")?;
        println!("Sample configuration file created. Copy it to one of:");
        println!("  - ./fogwatch.toml (project-specific)");
        println!("  - ~/.config/fogwatch/config.toml (user-specific)");
        return Ok(());
    }

    // Load configuration
    let mut config = Config::load()?;

    // Track credential sources for user feedback
    let mut token_source = "default";
    let mut account_id_source = "default";

    // Check environment variables
    if let Ok(env_token) = std::env::var("CLOUDFLARE_API_TOKEN") {
        config.auth.api_token = Some(env_token);
        token_source = "environment";
    }
    if let Ok(env_account_id) = std::env::var("CLOUDFLARE_ACCOUNT_ID") {
        config.auth.account_id = Some(env_account_id);
        account_id_source = "environment";
    }

    // Override with command line arguments if provided
    let token = if args.token.is_some() {
        token_source = "command line";
        args.token.clone()
    } else {
        if config.auth.api_token.is_some() {
            token_source = "config file";
        }
        config.auth.api_token.clone()
    }
    .context("API token not provided")?;

    let account_id = if args.account_id.is_some() {
        account_id_source = "command line";
        args.account_id.clone()
    } else {
        if config.auth.account_id.is_some() {
            account_id_source = "config file";
        }
        config.auth.account_id.clone()
    }
    .context("Account ID not provided")?;

    // Show credential sources
    println!("Using credentials from:");
    println!("  API token    : {token_source}");
    println!("  Account ID   : {account_id_source}");
    println!();

    // Create HTTP client
    let client = reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("Invalid API token format")?,
            );
            headers
        })
        .build()
        .context("Failed to create HTTP client")?;

    // Run TUI with merged configuration
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let (tx, rx) = mpsc::channel();
    let rt = tokio::runtime::Runtime::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    // Before run_ui call, fetch the workers
    let workers =
        match rt.block_on(async { fetch_workers(&client, &account_id, args.debug, &tx).await }) {
            Ok(workers) => workers.into_iter().map(|w| w.id).collect::<Vec<_>>(),
            Err(e) => {
                eprintln!("Failed to fetch workers: {e}");
                vec![]
            }
        };

    let ui_result = run_ui(
        &rt,
        &mut terminal,
        App::new(workers), // Pass the fetched workers
        rx,
        &client,
        &account_id,
        &token,
        &args,
        &config,
        tx,
    );

    // Cleanup terminal state
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // After cleanup, check and propagate any errors from the UI
    ui_result?;

    Ok(())
}
