# fogwatch Architecture

This document describes the architecture and design decisions of fogwatch.

## Overview

fogwatch is a terminal-based log viewer for Cloudflare Workers that provides real-time log streaming, filtering, and analytics. The application is built with a modular architecture focusing on:

- Real-time data processing
- Interactive terminal UI
- Efficient memory usage
- Extensible filtering system

## Core Components

### 1. Terminal UI (src/tui.rs)

The UI layer is built using the ratatui library and follows a Model-View pattern:

```rust
pub struct App {
    // Application state
    pub logs: Vec<TailEventMessage>,
    pub filtered_logs: Vec<usize>,
    pub state: AppState,
    // UI state
    pub list_state: ListState,
    pub log_state: ListState,
    // Analytics
    pub status_counts: StatusCounts,
}
```

Key UI components:
- Worker selection list
- Log viewer with filtering
- Status code frequency chart
- Detail view with JSON formatting

### 2. Data Types (src/types.rs)

Core data structures that define the application's domain model:

```rust
pub struct TailEventMessage {
    pub outcome: Outcome,
    pub script_name: Option<String>,
    pub logs: Vec<LogEntry>,
    pub event: Option<serde_json::Value>,
    // ...
}

pub enum StatusFilter {
    Success,    // 2xx
    ClientErr,  // 4xx
    ServerErr,  // 5xx
    Other,      // Other codes
    All,        // No filter
}
```

### 3. Configuration (src/config.rs)

Handles application configuration from multiple sources:
- Command line arguments
- Environment variables
- Configuration files (TOML)

### 4. WebSocket Client

Manages real-time communication with Cloudflare's API:
- Creates tail sessions
- Maintains WebSocket connection
- Handles reconnection
- Processes messages

## Data Flow

1. **Initialization**
   ```
   CLI Args/Config → App State → Worker Selection
   ```

2. **Log Streaming**
   ```
   WebSocket → Message Processing → Filtering → UI Update
   ```

3. **User Interaction**
   ```
   Keyboard Input → State Update → Filter Application → UI Refresh
   ```

## Design Decisions

### 1. Memory Management

- **Log Buffer**
  - Uses a Vec for log storage
  - Maintains indices for filtered views
  - Avoids cloning log entries

- **Message Processing**
  - Processes messages in chunks
  - Uses channels for async communication
  - Implements backpressure handling

### 2. UI Architecture

- **Component Separation**
  ```rust
  fn render_ui(f: &mut Frame<'_>, app: &mut App) {
      render_worker_list(f, app, chunks[0]);
      render_logs(f, app, chunks[1]);
      render_status_bar(f, app, chunks[2]);
  }
  ```

- **State Management**
  - Centralized app state
  - Immutable state updates
  - Clear state transitions

### 3. Filtering System

- **Two-Layer Filtering**
  1. Server-side (Cloudflare API)
  2. Client-side (Local filtering)

- **Filter Implementation**
  ```rust
  pub fn apply_filter(&mut self) {
      self.filtered_logs.clear();
      for (index, log) in self.logs.iter().enumerate() {
          if matches_filter(log, &self.active_filter) {
              self.filtered_logs.push(index);
          }
      }
  }
  ```

### 4. Error Handling

- Uses anyhow for error propagation
- Implements custom error types
- Provides detailed error context
- Handles WebSocket reconnection

## Performance Considerations

### 1. Memory Usage

- **Log Storage**
  - Implements circular buffer
  - Cleans up old entries
  - Uses references where possible

- **UI Rendering**
  - Minimizes allocations
  - Reuses buffers
  - Implements partial updates

### 2. Processing Efficiency

- **Message Handling**
  - Batches updates
  - Uses async processing
  - Implements throttling

- **Filtering**
  - Uses index-based filtering
  - Caches filter results
  - Optimizes common cases

## Future Architecture

### 1. Planned Improvements

- **Analytics Engine**
  ```rust
  pub struct Analytics {
      metrics: HashMap<String, Metric>,
      aggregates: Vec<Aggregate>,
      alerts: Vec<Alert>,
  }
  ```

- **Enhanced Filtering**
  ```rust
  pub enum FilterExpression {
      And(Box<FilterExpression>, Box<FilterExpression>),
      Or(Box<FilterExpression>, Box<FilterExpression>),
      Not(Box<FilterExpression>),
      Condition(FilterCondition),
  }
  ```

### 2. Scalability Considerations

- **Log Volume**
  - Implement log rotation
  - Add compression
  - Support external storage

- **UI Performance**
  - Add virtualization
  - Implement lazy loading
  - Optimize rendering

## Testing Strategy

### 1. Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_filter_application() {
        // Test filter logic
    }

    #[test]
    fn test_log_processing() {
        // Test message handling
    }
}
```

### 2. Integration Tests

- WebSocket connection tests
- UI interaction tests
- Configuration parsing tests
- End-to-end workflow tests

## Dependencies

Key external dependencies and their roles:
- ratatui: Terminal UI framework
- tokio: Async runtime
- serde: Serialization
- anyhow: Error handling
- clap: CLI parsing

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for:
- Development setup
- Coding guidelines
- PR process
- Testing requirements 