# Contributing to fogwatch

Thank you for your interest in contributing to fogwatch! This document provides guidelines and information for contributors.

## Development Setup

1. **Prerequisites**
   - Rust toolchain (latest stable)
   - Cargo package manager
   - Git

2. **Clone and Build**
   ```bash
   git clone https://github.com/yourusername/fogwatch.git
   cd fogwatch
   cargo build
   ```

3. **Run Tests**
   ```bash
   cargo test
   ```

## Project Structure

```
fogwatch/
├── src/
│   ├── main.rs       # Entry point and CLI handling
│   ├── tui.rs        # Terminal UI implementation
│   ├── types.rs      # Core data structures
│   └── config.rs     # Configuration handling
├── tests/            # Integration tests
└── examples/         # Usage examples
```

### Key Components

1. **Terminal UI (tui.rs)**
   - Uses ratatui for the terminal interface
   - Implements the interactive UI components
   - Handles keyboard input and navigation
   - Manages the display of logs and statistics

2. **Types (types.rs)**
   - Defines core data structures
   - Implements serialization/deserialization
   - Contains filter and message types

3. **Configuration (config.rs)**
   - Handles config file parsing
   - Manages environment variables
   - Implements configuration validation

## Code Style

1. **Formatting**
   - Use `cargo fmt` before committing
   - Follow standard Rust formatting guidelines

2. **Documentation**
   - Document all public functions and types
   - Include examples in doc comments
   - Keep comments up to date

3. **Error Handling**
   - Use `anyhow` for error propagation
   - Provide descriptive error messages
   - Handle all potential error cases

## Testing

1. **Unit Tests**
   - Write tests for all new functionality
   - Use descriptive test names
   - Test edge cases and error conditions

2. **Integration Tests**
   - Add tests for new features
   - Test real-world usage scenarios
   - Verify UI behavior

## Pull Request Process

1. **Before Creating a PR**
   - Create an issue for discussion
   - Fork the repository
   - Create a feature branch

2. **PR Requirements**
   - Clear description of changes
   - Tests for new functionality
   - Documentation updates
   - Clean commit history

3. **Review Process**
   - Address review comments
   - Keep PR scope focused
   - Ensure CI passes

## Development Guidelines

### Adding New Features

1. **UI Components**
   ```rust
   // Example of adding a new UI component
   fn render_new_component(f: &mut Frame<'_>, app: &App, area: Rect) {
       // Implementation
   }
   ```

2. **Filters**
   ```rust
   // Example of adding a new filter
   pub enum FilterType {
       // Add new filter variant
   }
   ```

3. **Configuration**
   ```rust
   // Example of adding new config option
   pub struct Config {
       // Add new field
   }
   ```

### Performance Considerations

1. **Memory Management**
   - Implement buffer limits
   - Clean up old log entries
   - Use efficient data structures

2. **UI Performance**
   - Minimize redraws
   - Optimize render functions
   - Handle large log volumes

3. **WebSocket Handling**
   - Implement reconnection logic
   - Handle connection timeouts
   - Buffer messages efficiently

## Feature Implementation Guide

### Adding a New Filter

1. Update `types.rs`:
   ```rust
   pub enum FilterType {
       // Add new filter type
   }
   ```

2. Implement filter logic:
   ```rust
   impl App {
       fn apply_filter(&mut self) {
           // Add filter implementation
       }
   }
   ```

3. Add UI elements:
   ```rust
   fn render_filter_ui(f: &mut Frame<'_>, app: &App, area: Rect) {
       // Add UI components
   }
   ```

### Adding Analytics

1. Define metrics:
   ```rust
   pub struct Metrics {
       // Add metric fields
   }
   ```

2. Implement collection:
   ```rust
   impl App {
       fn update_metrics(&mut self) {
           // Update metrics
       }
   }
   ```

3. Add visualization:
   ```rust
   fn render_metrics(f: &mut Frame<'_>, app: &App, area: Rect) {
       // Render metrics UI
   }
   ```

## Future Development

### Planned Features

1. **Enhanced Filtering**
   - Implementation guide
   - Priority level
   - Required changes

2. **Analytics Dashboard**
   - Metrics to track
   - UI layout
   - Data collection

3. **Performance Optimizations**
   - Target areas
   - Benchmarking
   - Implementation plan

## Getting Help

- Create an issue for questions
- Join our Discord channel
- Check the documentation

## License

By contributing, you agree that your contributions will be licensed under the MIT License. 