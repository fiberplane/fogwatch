// Terminal User Interface (TUI) module for fogwatch
//
// Provides an interactive terminal interface for:
// - Selecting workers to monitor
// - Displaying real-time log streams
// - Filtering and searching log entries
// - Viewing detailed log information
// - Managing display preferences

// Import required dependencies for terminal manipulation, async runtime, and WebSocket communication
use std::io;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use crossterm::event::{Event, KeyCode};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use serde_json;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        handshake::client::{generate_key, Request},
        Message,
    },
};
use url::Url;

use crate::{Outcome, TailEventMessage};

/// Represents the different states of the application UI
#[derive(Debug)]
pub enum AppState {
    /// Displaying worker selection screen
    SelectingWorker(Vec<String>),
    /// Establishing connection to Cloudflare API
    Connecting,
    /// Displaying log stream
    Viewing,
}

impl AppState {
    /// Returns the list of workers for the current state
    ///
    /// # Returns
    /// * `&[String]` - List of workers
    fn get_workers(&self) -> &[String] {
        match self {
            AppState::SelectingWorker(workers) => workers,
            _ => &[],
        }
    }
}

/// Tracks frequencies of different HTTP status codes
#[derive(Default)]
pub struct StatusCounts {
    /// 2xx Success responses
    pub success: usize,
    /// 4xx Client Error responses
    pub client_err: usize,
    /// 5xx Server Error responses
    pub server_err: usize,
    /// Other status codes
    pub other: usize,
}

/// Available status code filters for log display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusFilter {
    /// Show only 2xx responses
    Success,
    /// Show only 4xx responses
    ClientErr,
    /// Show only 5xx responses
    ServerErr,
    /// Show other status codes
    Other,
    /// Show all responses
    All,
}

/// Main application state container
pub struct App {
    /// All received log messages
    pub logs: Vec<TailEventMessage>,
    /// Indices of logs matching current filter
    pub filtered_logs: Vec<usize>,
    /// State of the worker selection list
    pub list_state: ListState,
    /// State of the log display list
    pub log_state: ListState,
    /// Whether the application should exit
    pub should_quit: bool,
    /// Current UI state
    pub state: AppState,
    /// Currently selected worker
    pub selected_worker: Option<String>,
    /// Whether to show detailed log view
    pub show_details: bool,
    /// Scroll position in detail view
    pub detail_scroll: u16,
    /// Whether detail view has focus
    pub detail_focused: bool,
    /// Count of different status codes
    pub status_counts: StatusCounts,
    /// Currently active status filter
    pub active_filter: Option<StatusFilter>,
}

impl App {
    /// Creates a new application instance
    ///
    /// # Arguments
    /// * `workers` - List of available workers to monitor
    ///
    /// # Returns
    /// * `App` - Initialized application state
    pub fn new(workers: Vec<String>) -> App {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            logs: Vec::new(),
            filtered_logs: Vec::new(),
            list_state,
            log_state: ListState::default(),
            should_quit: false,
            state: AppState::SelectingWorker(workers),
            selected_worker: None,
            show_details: false,
            detail_scroll: 0,
            detail_focused: false,
            status_counts: StatusCounts::default(),
            active_filter: None,
        }
    }

    /// Returns the list of workers for the current state
    ///
    /// # Returns
    /// * `&[String]` - List of workers
    pub fn get_workers(&self) -> &[String] {
        self.state.get_workers()
    }

    /// Moves selection to next item in current list
    pub fn next(&mut self) {
        let workers = self.get_workers();
        if workers.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= workers.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Moves selection to previous item in current list
    pub fn previous(&mut self) {
        let workers = self.get_workers();
        if workers.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    workers.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Selects the currently highlighted worker
    pub fn select_worker(&mut self) {
        if let AppState::SelectingWorker(ref workers) = self.state {
            if let Some(selected) = self.list_state.selected() {
                if selected < workers.len() {
                    self.selected_worker = Some(workers[selected].clone());
                    self.state = AppState::Connecting;
                    self.active_filter = Some(StatusFilter::All); // Set default filter to All
                    self.apply_filter(); // Apply the filter
                }
            }
        }
    }

    /// Moves to next log entry
    pub fn next_log(&mut self) {
        if self.logs.is_empty() {
            return;
        }

        let i = match self.log_state.selected() {
            Some(i) => {
                if i >= self.logs.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.log_state.select(Some(i));
        self.detail_scroll = 0; // Reset scroll position
    }

    /// Moves to previous log entry
    pub fn previous_log(&mut self) {
        if self.logs.is_empty() {
            return;
        }

        let i = match self.log_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.logs.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.log_state.select(Some(i));
        self.detail_scroll = 0; // Reset scroll position
    }

    /// Toggles detailed view for current log entry
    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
        if !self.show_details {
            self.detail_focused = false;
        }
    }

    // Add scroll methods
    pub fn scroll_details_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub fn scroll_details_down(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_add(1);
    }

    pub fn scroll_details_page_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(10);
    }

    pub fn scroll_details_page_down(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_add(10);
    }

    // Add focus switching methods
    pub fn focus_logs(&mut self) {
        self.detail_focused = false;
    }

    pub fn focus_details(&mut self) {
        if self.show_details {
            self.detail_focused = true;
        }
    }

    /// Updates status code statistics for a new log entry
    ///
    /// # Arguments
    /// * `log` - New log entry to process
    pub fn update_status_counts(&mut self, log: &TailEventMessage) {
        if let Some(event) = &log.event {
            if let Some(response) = event.get("response") {
                if let Some(status) = response.get("status").and_then(|s| s.as_u64()) {
                    match status {
                        200..=299 => self.status_counts.success += 1,
                        400..=499 => self.status_counts.client_err += 1,
                        500..=599 => self.status_counts.server_err += 1,
                        _ => self.status_counts.other += 1,
                    }
                }
            }
        }
    }

    /// Applies current status filter to log list
    pub fn apply_filter(&mut self) {
        self.filtered_logs.clear();

        for (index, log) in self.logs.iter().enumerate() {
            let matches = match self.active_filter {
                Some(StatusFilter::Success) => {
                    if let Some(event) = &log.event {
                        if let Some(response) = event.get("response") {
                            if let Some(status) = response.get("status").and_then(|s| s.as_u64()) {
                                (200..300).contains(&status)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                Some(StatusFilter::ClientErr) => {
                    if let Some(event) = &log.event {
                        if let Some(response) = event.get("response") {
                            if let Some(status) = response.get("status").and_then(|s| s.as_u64()) {
                                (400..500).contains(&status)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                Some(StatusFilter::ServerErr) => {
                    if let Some(event) = &log.event {
                        if let Some(response) = event.get("response") {
                            if let Some(status) = response.get("status").and_then(|s| s.as_u64()) {
                                (500..600).contains(&status)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                Some(StatusFilter::Other) => {
                    // with edition 2024 let chains are stabilized and then this can be simplified
                    if let Some(event) = &log.event {
                        if let Some(response) = event.get("response") {
                            if let Some(status) = response.get("status").and_then(|s| s.as_u64()) {
                                !(200..600).contains(&status)
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                }
                Some(StatusFilter::All) | None => true,
            };

            if matches {
                self.filtered_logs.push(index);
            }
        }

        // Reset log selection if it's now out of bounds
        if let Some(selected) = self.log_state.selected() {
            if selected >= self.filtered_logs.len() {
                self.log_state.select(Some(0));
            }
        }
    }

    /// Sets the active status filter
    ///
    /// # Arguments
    /// * `filter` - New filter to apply
    pub fn set_filter(&mut self, filter: Option<StatusFilter>) {
        self.active_filter = filter;
        self.apply_filter();
    }

    /// Toggles the active status filter
    ///
    /// # Arguments
    /// * `filter` - Filter to toggle
    pub fn toggle_filter(&mut self, filter: StatusFilter) {
        if self.active_filter == Some(filter) {
            self.set_filter(None);
        } else {
            self.set_filter(Some(filter));
        }
    }
}

/// Main UI loop and event handling
pub fn run_ui(
    rt: &tokio::runtime::Runtime,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
    rx: mpsc::Receiver<TailEventMessage>,
    client: &reqwest::Client,
    account_id: &str,
    token: &str,
    args: &crate::Args,
    _config: &crate::config::Config,
    tx: mpsc::Sender<TailEventMessage>,
) -> Result<()> {
    // Clone arguments to avoid ownership issues in async blocks
    let args_clone = crate::Args {
        token: Some(token.to_string()),
        account_id: Some(account_id.to_string()),
        worker: None,
        method: args.method.clone(),
        search: args.search.clone(),
        status: args.status.clone(),
        sampling_rate: args.sampling_rate,
        debug: args.debug,
        format: args.format,
        init_config: false,
    };

    // Main event loop
    loop {
        // Draw the UI based on current state
        terminal.draw(|f| render_ui(f, &mut app))?;

        // Handle keyboard events
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                        break;
                    }
                    KeyCode::Char('2') => {
                        app.toggle_filter(StatusFilter::Success);
                    }
                    KeyCode::Char('4') => {
                        app.toggle_filter(StatusFilter::ClientErr);
                    }
                    KeyCode::Char('5') => {
                        app.toggle_filter(StatusFilter::ServerErr);
                    }
                    KeyCode::Char('0') => {
                        app.toggle_filter(StatusFilter::Other);
                    }
                    KeyCode::Char('a') => {
                        app.set_filter(None);
                    }
                    KeyCode::Enter => {
                        if let AppState::SelectingWorker(_) = app.state {
                            app.select_worker();
                        } else {
                            app.toggle_details();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let AppState::SelectingWorker(_) = app.state {
                            app.previous()
                        } else if app.show_details && app.detail_focused {
                            app.scroll_details_up();
                        } else {
                            app.previous_log()
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let AppState::SelectingWorker(_) = app.state {
                            app.next()
                        } else if app.show_details && app.detail_focused {
                            app.scroll_details_down();
                        } else {
                            app.next_log()
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if app.show_details {
                            app.focus_logs();
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if app.show_details {
                            app.focus_details();
                        }
                    }
                    KeyCode::PageUp | KeyCode::Char('u') => {
                        if app.show_details && app.detail_focused {
                            app.scroll_details_page_up();
                        }
                    }
                    KeyCode::PageDown | KeyCode::Char('d') => {
                        if app.show_details && app.detail_focused {
                            app.scroll_details_page_down();
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle state transitions and WebSocket connection
        match &app.state {
            AppState::Connecting => {
                if let Some(worker) = &app.selected_worker {
                    // Start WebSocket connection
                    let worker = worker.clone();
                    let tx_for_ws = tx.clone();
                    let tx_for_error = tx.clone();
                    let args_with_worker = crate::Args {
                        token: Some(token.to_string()),
                        account_id: Some(account_id.to_string()),
                        worker: Some(worker.clone()),
                        debug: args_clone.debug,
                        method: args_clone.method.clone(),
                        search: args_clone.search.clone(),
                        status: args_clone.status.clone(),
                        sampling_rate: args_clone.sampling_rate,
                        format: args_clone.format.clone(),
                        init_config: false,
                    };
                    let filters = crate::parse_filters(&args_with_worker);
                    match rt.block_on(async {
                        // Create tail session
                        let tail_info = crate::create_tail(
                            client,
                            &args_with_worker,
                            filters.clone(),
                            &tx_for_ws
                        )
                        .await?;

                        // Build WebSocket URL
                        let ws_url = tail_info.url.replace("http://", "wss://");

                        create_debug_log_entry(args_with_worker.debug, &tx_for_ws, format!("Attempting to connect to WebSocket URL: {ws_url}"), "debug");

                        // Connect to WebSocket
                        let url = Url::parse(&ws_url)?;
                        let ws_key = generate_key();
                        let request = Request::builder()
                            .uri(ws_url)
                            .header("Host", url.host_str().unwrap_or_default())
                            .header("User-Agent", format!("fogwatch/{}", env!("CARGO_PKG_VERSION")))
                            .header("Sec-WebSocket-Protocol", "trace-v1")
                            .header("Sec-WebSocket-Key", ws_key)
                            .header("Sec-WebSocket-Version", "13")
                            .header("Connection", "Upgrade")
                            .header("Upgrade", "websocket")
                            .body(())?;

                        create_debug_log_entry(args_with_worker.debug, &tx_for_ws, format!("Using trace-v1 protocol with host: {}", url.host_str().unwrap_or_default()), "debug");

                        // After WebSocket connection is established
                        let (ws_stream, _) = connect_async(request).await?;

                        create_debug_log_entry(args_with_worker.debug, &tx_for_ws, "WebSocket connection established, waiting for logs...", "debug");

                        let (write, mut read) = ws_stream.split();
                        let write = std::sync::Arc::new(tokio::sync::Mutex::new(write));
                        let write_for_ping = write.clone();

                        // Start ping/pong task
                        let ping_interval = tokio::time::interval(Duration::from_secs(10));
                        let tx_for_ping = tx_for_error.clone();
                        let args_for_ping = args_with_worker.clone();
                        let waiting_for_pong = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let waiting_for_pong_clone = waiting_for_pong.clone();
                        let _ping_task = tokio::spawn(async move {
                            let mut interval = ping_interval;
                            loop {
                                interval.tick().await;
                                if waiting_for_pong.load(std::sync::atomic::Ordering::SeqCst) {
                                    create_debug_log_entry(args_for_ping.debug, &tx_for_ping, "No pong received, connection may be dead", "error");

                                    break;
                                }
                                waiting_for_pong.store(true, std::sync::atomic::Ordering::SeqCst);
                                if let Err(e) = write_for_ping.lock().await.send(Message::Ping(vec![1, 2, 3, 4])).await {
                                    create_debug_log_entry(args_for_ping.debug, &tx_for_ping, format!("Ping failed: {e}"), "error");
                                    break;
                                }
                            }
                        });

                        // Start message reading task
                        let tx_for_read = tx_for_ws.clone();
                        let args_for_read = args_with_worker.clone();
                        let write_for_read = write.clone();
                        let _handle = tokio::spawn(async move {
                            create_debug_log_entry(args_for_read.debug, &tx_for_read, "Starting message reading task...", "debug");

                            while let Some(msg) = read.next().await {
                                create_debug_log_entry(args_for_read.debug, &tx_for_read, "Received a message from WebSocket stream", "debug");

                                match msg {
                                    Ok(msg) => {
                                        match msg {
                                            Message::Text(text) => {
                                                let msg = format!("Received raw WebSocket text message:\n  Raw content: {text}\n  Content length: {} bytes\n  Content type: Text",
                                                                  text.len()
                                                );

                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, msg, "debug");


                                                // Try parsing as JSON first
                                                match serde_json::from_str::<serde_json::Value>(&text) {
                                                    Ok(json) => {
                                                        create_debug_log_entry(
                                                            args_for_read.debug,
                                                            &tx_for_read,
                                                            format!("Successfully parsed as JSON:\n  {}", serde_json::to_string_pretty(&json).unwrap_or_default()),
                                                            "debug"
                                                        );

                                                    }
                                                    Err(e) => {
                                                        create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Failed to parse as JSON:\n  Error: {e}"), "debug");
                                                    }
                                                }

                                                // Try parsing as TailEventMessage
                                                match serde_json::from_str::<TailEventMessage>(&text) {
                                                    Ok(event) => {
                                                        let msg = format!("Successfully parsed as TailEventMessage:\n  Outcome: {:?}\n  Script: {:?}\n  Event: {}\n  Logs: {:?}\n  Exceptions: {:?}\n  Timestamp: {}",
                                                                          event.outcome,
                                                                          event.script_name,
                                                                          serde_json::to_string_pretty(&event.event).unwrap_or_default(),
                                                                          event.logs,
                                                                          event.exceptions,
                                                                          event.event_timestamp
                                                        );
                                                        create_debug_log_entry(args_for_read.debug, &tx_for_read, msg, "debug");

                                                        let _ = tx_for_read.send(event);
                                                    }
                                                    Err(e) => {
                                                        let msg = format!("Failed to parse as TailEventMessage:\n  Raw message: {}\n  Error: {}\n  Error kind: {:?}\n  Error category: {:?}",
                                                                          text,
                                                                          e,
                                                                          e.classify(),
                                                                          e.to_string()
                                                        );

                                                        create_debug_log_entry(args_for_read.debug, &tx_for_read, msg, "error");
                                                    }
                                                }
                                            }
                                            Message::Binary(data) => {
                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Received binary message of {} bytes", data.len()), "debug");

                                                // Try to parse binary data as UTF-8 string first
                                                match String::from_utf8(data) {
                                                    Ok(text) => {
                                                        create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Successfully converted binary to text:\n{text}"), "debug");

                                                        // Try parsing as TailEventMessage
                                                        match serde_json::from_str::<TailEventMessage>(&text) {
                                                            Ok(event) => {
                                                                let msg = format!("Successfully parsed binary as TailEventMessage:\n  Outcome: {:?}\n  Script: {:?}\n  Event: {}\n  Logs: {:?}\n  Exceptions: {:?}\n  Timestamp: {}",
                                                                                  event.outcome,
                                                                                  event.script_name,
                                                                                  serde_json::to_string_pretty(&event.event).unwrap_or_default(),
                                                                                  event.logs,
                                                                                  event.exceptions,
                                                                                  event.event_timestamp
                                                                );

                                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, msg, "debug");

                                                                let _ = tx_for_read.send(event);
                                                            }
                                                            Err(e) => {
                                                                let msg = format!("Failed to parse binary as TailEventMessage:\n  Text: {}\n  Error: {}\n  Error kind: {:?}\n  Error category: {:?}",
                                                                                  text,
                                                                                  e,
                                                                                  e.classify(),
                                                                                  e.to_string()
                                                                );

                                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, msg, "error");
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Failed to convert binary to text: {e}"), "error");
                                                    }
                                                }
                                            }
                                            Message::Ping(data) => {
                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Received ping with {} bytes of data", data.len()), "debug");

                                                if let Err(e) = write_for_read.lock().await.send(Message::Pong(data)).await {
                                                    create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Failed to send pong: {e}"), "error");
                                                }
                                            }
                                            Message::Pong(_) => {
                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, "Received pong from server", "debug");

                                                waiting_for_pong_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                                            }
                                            Message::Close(frame) => {
                                                let reason = frame.map(|f| format!("code: {}, reason: {}", f.code, f.reason)).unwrap_or_default();

                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Received close frame from server: {reason}"), "debug");
                                            }
                                            Message::Frame(frame) => {
                                                create_debug_log_entry(args_for_read.debug, &tx_for_read, format!("Received raw frame: {frame:?}"), "debug");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        create_debug_log_entry(
                                            args_for_read.debug,
                                            &tx_for_read,
                                            format!("WebSocket error:\n  Error: {e}\n  Error kind: {:?}", e.to_string()),
                                            "error"
                                        );

                                        break;
                                    }
                                }
                            }

                            create_debug_log_entry(args_for_read.debug, &tx_for_read, "Message reading task ended", "debug");
                        });

                        let tx_for_final = tx_for_ws.clone();

                        create_debug_log_entry(args_with_worker.debug, &tx_for_final, "WebSocket connection established, waiting for logs...", "debug");

                        Ok::<_, anyhow::Error>(())
                    }) {
                        Ok(_) => {
                            // Successfully connected, transition to viewing state
                            app.state = AppState::Viewing;
                        }
                        Err(e) => {
                            // Connection failed, show error and return to worker selection
                            create_debug_log_entry(args_clone.debug, &tx_for_error, format!("Failed to connect: {e}"), "error");
                            app.state = AppState::SelectingWorker(app.get_workers().to_vec());
                        }
                    }
                }
            }
            _ => {}
        }

        // Check for new log messages
        if let Ok(log) = rx.try_recv() {
            app.update_status_counts(&log); // Update status counts before adding the log
            app.logs.push(log);
            app.apply_filter(); // Re-apply filter after adding new log
        }

        // Check if we should exit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn create_debug_log_entry(
    debug: bool,
    sender: &Sender<TailEventMessage>,
    msg: impl Into<String>,
    level: &str,
) {
    if !debug {
        return;
    }

    let log = crate::create_log_entry(msg.into(), level);
    let _ = sender.send(crate::create_tail_event(log));
}

#[allow(dead_code)]
fn format_json_value(value: &serde_json::Value, base_indent: usize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(formatted) => {
            let mut result = String::new();
            let mut first_line = true;

            for line in formatted.lines() {
                if first_line {
                    // First line (usually just '{' or '[') gets base indentation
                    result.push_str(&format!("{:indent$}{}\n", "", line, indent = base_indent));
                    first_line = false;
                } else {
                    // All subsequent lines get base + 2 indentation to align with JSON structure
                    let content = line.trim_start();
                    // Add 2 spaces of indentation for each level of JSON nesting
                    let additional_indent = line.chars().take_while(|c| c.is_whitespace()).count();
                    result.push_str(&format!(
                        "{:indent$}{}\n",
                        "",
                        content,
                        indent = base_indent + additional_indent * 2
                    ));
                }
            }
            // Remove the last newline
            if result.ends_with('\n') {
                result.pop();
            }
            result
        }
        Err(_) => format!("{:indent$}{}", "", value.to_string(), indent = base_indent),
    }
}

/// Renders the worker selection list
///
/// # Arguments
/// * `f` - Frame to render into
/// * `app` - Application state
/// * `area` - Screen area to use
fn render_worker_list(f: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app
        .get_workers()
        .iter()
        .map(|worker| ListItem::new(worker.as_str()).style(Style::default().fg(Color::White)))
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Worker List").borders(Borders::NONE))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

    f.render_stateful_widget(list, area, &mut app.list_state.clone());
}

/// Renders the log display
///
/// # Arguments
/// * `f` - Frame to render into
/// * `app` - Application state
/// * `area` - Screen area to use
fn render_logs(f: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    // Split area vertically for frequency chart and logs
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Height for frequency chart
            Constraint::Min(0),    // Remaining space for logs
        ])
        .split(area);

    // Render frequency chart
    let total_requests = app.status_counts.success
        + app.status_counts.client_err
        + app.status_counts.server_err
        + app.status_counts.other;

    let chart_text = if total_requests > 0 {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("2xx: {:<5}", app.status_counts.success),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" | "),
                Span::styled(
                    format!("4xx: {:<5}", app.status_counts.client_err),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(" | "),
                Span::styled(
                    format!("5xx: {:<5}", app.status_counts.server_err),
                    Style::default().fg(Color::Red),
                ),
                Span::raw(" | "),
                Span::styled(
                    format!("Other: {:<5}", app.status_counts.other),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(format!(" | Total: {total_requests}")),
            ]),
            Line::from(vec![
                Span::styled(
                    "█".repeat((app.status_counts.success * 50 / total_requests).max(1)),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(
                    "█".repeat((app.status_counts.client_err * 50 / total_requests).max(1)),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    "█".repeat((app.status_counts.server_err * 50 / total_requests).max(1)),
                    Style::default().fg(Color::Red),
                ),
                Span::styled(
                    "█".repeat((app.status_counts.other * 50 / total_requests).max(1)),
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]
    } else {
        vec![Line::from(vec![Span::raw("No requests recorded yet")])]
    };

    let chart = Paragraph::new(Text::from(chart_text)).block(
        Block::default()
            .borders(Borders::NONE)
            .style(Style::default()),
    );

    f.render_widget(chart, chunks[0]);

    // Split remaining area for logs and details if details are shown
    let log_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if app.show_details {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        } else {
            vec![Constraint::Percentage(100)]
        })
        .split(chunks[1]);

    // Render main log list
    let logs: Vec<ListItem> = app
        .filtered_logs
        .iter()
        .filter_map(|&index| app.logs.get(index))
        .map(|log| {
            let outcome_str = match log.outcome {
                Outcome::Ok => "OK",
                Outcome::Exception => "ERROR",
                Outcome::Canceled => "CANCELED",
                Outcome::ExceededCpu => "CPU_LIMIT",
                Outcome::ExceededMemory => "MEM_LIMIT",
                Outcome::Unknown => "UNKNOWN",
            };

            let event_info = if let Some(event) = &log.event {
                if let Some(request) = event.get("request") {
                    let method = request
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("UNKNOWN");
                    let url = request
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("UNKNOWN");
                    let status = event
                        .get("response")
                        .and_then(|r| r.get("status"))
                        .and_then(|s| s.as_u64());

                    let style = if let Some(status_code) = status {
                        match status_code {
                            200..=299 => Style::default().fg(Color::Green),
                            400..=499 => Style::default().fg(Color::Yellow),
                            500..=599 => Style::default().fg(Color::Red),
                            _ => Style::default().fg(Color::White),
                        }
                    } else {
                        Style::default().fg(Color::White)
                    };

                    (
                        format!(
                            "{} {} ({})",
                            method,
                            url,
                            status
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "---".to_string())
                        ),
                        style,
                    )
                } else if let Some(cron) = event.get("cron") {
                    (
                        format!("CRON: {}", cron.as_str().unwrap_or("UNKNOWN")),
                        Style::default().fg(Color::Cyan),
                    )
                } else if let Some(queue) = event.get("queue") {
                    (
                        format!("QUEUE: {}", queue.as_str().unwrap_or("UNKNOWN")),
                        Style::default().fg(Color::Magenta),
                    )
                } else {
                    ("EVENT".to_string(), Style::default().fg(Color::White))
                }
            } else {
                ("".to_string(), Style::default().fg(Color::White))
            };

            let message = log
                .logs
                .iter()
                .map(|l| {
                    format!(
                        "[{}] {}",
                        l.level,
                        l.message
                            .iter()
                            .map(|m| m.to_string().trim_matches('"').to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            let timestamp = DateTime::<Utc>::from_timestamp(log.event_timestamp, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S");

            let display_text = if !event_info.0.is_empty() {
                format!(
                    "{} | {} | {} | {}",
                    timestamp, outcome_str, event_info.0, message
                )
            } else {
                format!("{} | {} | {}", timestamp, outcome_str, message)
            };

            ListItem::new(display_text).style(event_info.1)
        })
        .collect();

    let logs_list = List::new(logs)
        .block(
            Block::default()
                .title(if !app.detail_focused {
                    "Logs (focused)"
                } else {
                    "Logs"
                })
                .borders(Borders::NONE)
                .border_type(BorderType::Plain),
        )
        .style(if !app.detail_focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

    f.render_stateful_widget(logs_list, log_chunks[0], &mut app.log_state.clone());

    // Render detail view if enabled
    if app.show_details {
        if let Some(selected) = app.log_state.selected() {
            if let Some(log) = app.logs.get(selected) {
                let mut detail_text = Vec::new();

                // Add timestamp
                let timestamp = DateTime::<Utc>::from_timestamp(log.event_timestamp, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S%.3f");
                detail_text.push(format!("Timestamp: {timestamp}"));

                // Add outcome
                detail_text.push(format!("Outcome: {:?}", log.outcome));

                // Add script name if present
                if let Some(script) = &log.script_name {
                    detail_text.push(format!("Script: {script}"));
                }

                // Add event details with proper JSON formatting
                if let Some(event) = &log.event {
                    detail_text.push("\nEvent Details:\n".to_string());
                    let json_value = serde_json::json!(event);

                    fn format_json(value: &serde_json::Value, depth: usize) -> String {
                        let indent = "\u{00A0}\u{00A0}\u{00A0}\u{00A0}".repeat(depth);

                        match value {
                            serde_json::Value::Object(map) => {
                                let mut lines = Vec::new();
                                lines.push(format!("{}{{", indent));

                                for (k, v) in map {
                                    match v {
                                        serde_json::Value::Object(_) => {
                                            lines.push(format!(
                                                "{}{}\"{}\": {{",
                                                indent, "\u{00A0}\u{00A0}\u{00A0}\u{00A0}", k
                                            ));
                                            lines.push(format_json(v, depth + 1));
                                            lines.push(format!(
                                                "{}{}}},",
                                                indent, "\u{00A0}\u{00A0}\u{00A0}\u{00A0}"
                                            ));
                                        }
                                        _ => {
                                            let value_str = match v {
                                                serde_json::Value::String(s) => {
                                                    format!("\"{}\"", s)
                                                }
                                                serde_json::Value::Number(n) => n.to_string(),
                                                serde_json::Value::Bool(b) => b.to_string(),
                                                serde_json::Value::Null => "null".to_string(),
                                                _ => v.to_string(),
                                            };
                                            lines.push(format!(
                                                "{}{}\"{}\": {},",
                                                indent,
                                                "\u{00A0}\u{00A0}\u{00A0}\u{00A0}",
                                                k,
                                                value_str
                                            ));
                                        }
                                    }
                                }

                                lines.push(format!("{}}}", indent));
                                lines.join("\n")
                            }
                            _ => value.to_string(),
                        }
                    }

                    let formatted = format_json(&json_value, 1);
                    detail_text.push(formatted);
                    detail_text.push("\n".to_string());
                }

                // Add logs with proper indentation
                if !log.logs.is_empty() {
                    detail_text.push("\nLogs:\n".to_string());
                    for log_entry in &log.logs {
                        detail_text.push(format!("    [{}]", log_entry.level));
                        for message in &log_entry.message {
                            if let Ok(json_str) = serde_json::to_string_pretty(message) {
                                let formatted_json = json_str
                                    .lines()
                                    .map(|line| format!("        {line}"))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                detail_text.push(formatted_json);
                            }
                        }
                    }
                }

                // Add exceptions with proper indentation
                if !log.exceptions.is_empty() {
                    detail_text.push("\nExceptions:\n".to_string());
                    for exception in &log.exceptions {
                        detail_text.push(format!("    {}: ", exception.name));
                        if let Ok(json_str) = serde_json::to_string_pretty(&exception.message) {
                            let formatted_json = json_str
                                .lines()
                                .map(|line| format!("        {line}"))
                                .collect::<Vec<_>>()
                                .join("\n");
                            detail_text.push(formatted_json);
                        }
                    }
                }

                let detail_text = detail_text.join("\n");
                let mut lines = Vec::new();

                // Convert the text into styled spans
                for line in detail_text.lines() {
                    let mut line_spans = Vec::new();
                    let mut chars = line.chars().peekable();

                    while let Some(c) = chars.next() {
                        match c {
                            '{' | '}' | ':' | ',' => {
                                line_spans.push(Span::styled(
                                    c.to_string(),
                                    Style::default().fg(Color::Cyan),
                                ));
                            }
                            '"' => {
                                let mut content = String::new();
                                content.push('"');

                                while let Some(next_char) = chars.next() {
                                    content.push(next_char);
                                    if next_char == '"' {
                                        break;
                                    }
                                }

                                // If this is a key (followed by a colon)
                                if chars.peek().map_or(false, |&c| c == ':') {
                                    line_spans.push(Span::styled(
                                        content,
                                        Style::default().fg(Color::Magenta),
                                    ));
                                } else {
                                    line_spans.push(Span::styled(
                                        content,
                                        Style::default().fg(Color::Green),
                                    ));
                                }
                            }
                            c if c.is_ascii_digit() || c == '-' || c == '.' => {
                                let mut num = String::new();
                                num.push(c);
                                while let Some(&next_char) = chars.peek() {
                                    if next_char.is_ascii_digit() || next_char == '.' {
                                        num.push(chars.next().unwrap());
                                    } else {
                                        break;
                                    }
                                }
                                line_spans
                                    .push(Span::styled(num, Style::default().fg(Color::Yellow)));
                            }
                            c if c.is_whitespace() => {
                                line_spans.push(Span::raw(c.to_string()));
                            }
                            _ => {
                                let mut word = String::new();
                                word.push(c);
                                while let Some(&next_char) = chars.peek() {
                                    if !next_char.is_whitespace()
                                        && next_char != '"'
                                        && next_char != '{'
                                        && next_char != '}'
                                        && next_char != ':'
                                        && next_char != ','
                                    {
                                        word.push(chars.next().unwrap());
                                    } else {
                                        break;
                                    }
                                }

                                let style = match word.as_str() {
                                    "true" | "false" => Style::default().fg(Color::Blue),
                                    "null" => Style::default().fg(Color::DarkGray),
                                    _ => Style::default(),
                                };
                                line_spans.push(Span::styled(word, style));
                            }
                        }
                    }
                    lines.push(Line::from(line_spans));
                }

                let paragraph = Paragraph::new(Text::from(lines))
                    .block(
                        Block::default()
                            .title(if app.detail_focused {
                                "Details (focused)"
                            } else {
                                "Details"
                            })
                            .borders(Borders::NONE),
                    )
                    .wrap(Wrap { trim: true })
                    .scroll((app.detail_scroll, 0))
                    .style(if app.detail_focused {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    });

                f.render_widget(
                    paragraph.block(
                        Block::default()
                            .style(Style::default())
                            .borders(Borders::NONE),
                    ),
                    log_chunks[1],
                );
            }
        }
    }
}

/// Renders the status bar
///
/// # Arguments
/// * `f` - Frame to render into
/// * `app` - Application state
/// * `area` - Screen area to use
fn render_status_bar(f: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let mut status_text = match &app.state {
        AppState::SelectingWorker(_) => vec![
            Span::raw("Status: "),
            Span::styled("Selecting worker", Style::default().fg(Color::Yellow)),
        ],
        AppState::Connecting => vec![
            Span::raw("Status: "),
            Span::styled("Connecting", Style::default().fg(Color::Yellow)),
            Span::raw(" to worker "),
            Span::styled(
                app.selected_worker.as_deref().unwrap_or("unknown"),
                Style::default().fg(Color::Cyan),
            ),
        ],
        AppState::Viewing => vec![
            Span::raw("Status: "),
            Span::styled("Connected", Style::default().fg(Color::Green)),
            Span::raw(" to worker "),
            Span::styled(
                app.selected_worker.as_deref().unwrap_or("unknown"),
                Style::default().fg(Color::Cyan),
            ),
        ],
    };

    // Add filter information
    if let Some(filter) = app.active_filter {
        status_text.extend_from_slice(&[
            Span::raw(" | Filter: "),
            Span::styled(
                match filter {
                    StatusFilter::Success => "2xx",
                    StatusFilter::ClientErr => "4xx",
                    StatusFilter::ServerErr => "5xx",
                    StatusFilter::Other => "Other",
                    StatusFilter::All => "All",
                },
                Style::default().fg(Color::Yellow),
            ),
        ]);
    }

    let status_line = Line::from(status_text);
    let status_bar = Paragraph::new(status_line)
        .style(Style::default().bg(Color::Black))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(status_bar, area);
}

/// Main UI rendering function
///
/// # Arguments
/// * `f` - Frame to render into
/// * `app` - Application state
fn render_ui(f: &mut Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Worker list
            Constraint::Min(0),     // Logs
            Constraint::Length(1),  // Status bar
        ])
        .split(f.size());

    match app.state {
        AppState::SelectingWorker(_) => {
            render_worker_list(f, app, chunks[0]);
            render_logs(f, app, chunks[1]);
            render_status_bar(f, app, chunks[2]);
        }
        AppState::Viewing => {
            render_logs(f, app, chunks[0].union(chunks[1]));
            render_status_bar(f, app, chunks[2]);
        }
        AppState::Connecting => {
            render_logs(f, app, chunks[0].union(chunks[1]));
            render_status_bar(f, app, chunks[2]);
        }
    }
}
