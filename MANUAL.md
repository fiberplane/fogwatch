# Fogwatch Manual

Fogwatch is a Terminal User Interface (TUI) application for monitoring and debugging Cloudflare Workers in real-time. It provides a clean, intuitive interface for viewing worker logs, filtering events, and inspecting detailed information.

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/fogwatch.git
cd fogwatch

# Build the project
cargo build --release

# The binary will be available at
./target/release/fogwatch
```

## Authentication

Fogwatch requires Cloudflare API credentials to access your workers. You can provide these in two ways:

1. Environment variables:
```bash
export CLOUDFLARE_API_TOKEN="your-api-token"
export CLOUDFLARE_ACCOUNT_ID="your-account-id"
```

2. Command line arguments:
```bash
fogwatch --token "your-api-token" --account-id "your-account-id"
```

## Basic Usage

```bash
# Start monitoring all workers
fogwatch

# Monitor specific workers
fogwatch --workers worker1,worker2

# Show help
fogwatch --help
```

## Command Line Options

```
USAGE:
    fogwatch [OPTIONS]

OPTIONS:
    -t, --token <TOKEN>            Cloudflare API token [env: CLOUDFLARE_API_TOKEN=]
    -a, --account-id <ACCOUNT_ID>  Cloudflare account ID [env: CLOUDFLARE_ACCOUNT_ID=]
    -w, --workers <WORKERS>        Comma-separated list of worker names to monitor
    -h, --help                     Print help information
    -V, --version                  Print version information
```

## TUI Navigation

### General Controls
- `q`: Quit the application
- `Tab`: Switch focus between worker list and log details
- `↑/↓`: Navigate through lists
- `Enter`: Select a worker or log entry

### Log Panel Controls
- `PageUp/PageDown`: Scroll through log details
- `Home/End`: Jump to start/end of log details

### Log Level Filtering
The log panel shows the current filter in its title. Available filters:
- `a`: Show all log entries
- `2`: Show only 2xx logs
- `4`: Show only 4xx logs
- `5`: Show only 5xx logs

### Worker List Controls
- `↑/↓`: Navigate through workers
- `Enter`: Select a worker to view its logs

### Detail View
The detail view shows comprehensive information about the selected log entry:
- Timestamp
- Worker name
- Event type
- Request details
- Response information
- Error messages (if any)
- JSON-formatted event data with syntax highlighting

## Log Format

Each log entry contains:
- Timestamp
- Worker ID
- Script name
- Outcome (Success/Error)
- Event details in JSON format

Example log entry:
```json
{
  "timestamp": "2025-01-13T22:00:00Z",
  "worker": "my-worker",
  "event": {
    "request": {
      "url": "https://example.com/api",
      "method": "GET"
    },
    "response": {
      "status": 200
    }
  }
}
```

## Tips and Best Practices

1. **Efficient Monitoring**:
   - Use worker filters to focus on specific workers when debugging
   - Use log level filters to reduce noise and focus on important events

2. **Performance**:
   - The TUI efficiently handles large volumes of logs
   - Use log level filtering to reduce memory usage when monitoring for long periods

3. **Debugging**:
   - Use the detailed view to inspect full request/response cycles
   - JSON formatting makes it easy to spot patterns and issues

## Troubleshooting

Common issues and solutions:

1. **Authentication Errors**:
   ```
   Error: Failed to authenticate
   ```
   - Verify your API token has the correct permissions
   - Check that your account ID is correct
   - Ensure your token hasn't expired

2. **No Logs Appearing**:
   - Verify the worker(s) are actively receiving requests
   - Check if the worker names are correct
   - Ensure your API token has permission to read worker logs

3. **UI Issues**:
   - Ensure your terminal supports UTF-8
   - Try resizing your terminal window
   - Check that your terminal supports 256 colors

## Support

For bug reports and feature requests, please open an issue on the GitHub repository.

## Contributing

Contributions are welcome! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) file for guidelines.
