# fogwatch 🌫️

A real-time log viewer for Cloudflare Workers with filtering and multiple output formats.

## Features

- 🔍 **Real-time Log Streaming**: Watch your Worker logs as they happen
- 🎨 **Interactive TUI**: Terminal-based user interface with vim-like navigation
- 📊 **Status Code Analytics**: Visual frequency chart for HTTP status codes
- 🎯 **Advanced Filtering**: Filter logs by status codes (2xx, 4xx, 5xx)
- 🎯 **Multiple Output Formats**: Pretty, JSON, and compact formats
- 🛠️ **Configurable**: Command-line flags and config file support

## Installation

```bash
cargo install fogwatch
```

## Quick Start

1. Set your Cloudflare credentials:
```bash
export CLOUDFLARE_API_TOKEN="your-api-token"
export CLOUDFLARE_ACCOUNT_ID="your-account-id"
```

2. Run fogwatch:
```bash
fogwatch
```

## Interactive UI Navigation

### Keyboard Shortcuts
- **Worker Selection**:
  - `↑`/`k`: Move selection up
  - `↓`/`j`: Move selection down
  - `Enter`: Select worker

- **Log Navigation**:
  - `↑`/`k`: Scroll logs up
  - `↓`/`j`: Scroll logs down
  - `Enter`: Toggle details view
  - `h`/`l`: Switch focus between logs and details
  - `q`: Quit

- **Status Code Filtering**:
  - `2`: Show only 2xx responses
  - `4`: Show only 4xx responses
  - `5`: Show only 5xx responses
  - `0`: Show other status codes
  - `a`: Show all logs

### UI Elements
- **Top Bar**: Status code frequency chart
- **Main Area**: Log entries with color-coded status
- **Status Bar**: Connection status and active filter

## Command Line Options

```bash
USAGE:
    fogwatch [OPTIONS]

OPTIONS:
    -t, --token <TOKEN>            Cloudflare API token
    -a, --account-id <ACCOUNT_ID>  Cloudflare Account ID
    -w, --worker <WORKER>          Worker name to monitor
    -m, --method <METHOD>...       Filter by HTTP method
    -s, --status <STATUS>...       Filter by status (ok, error, canceled)
    -q, --search <SEARCH>          Search string in logs
    -r, --sampling-rate <RATE>     Sampling rate (0.0 to 1.0)
    -f, --format <FORMAT>          Output format (pretty, json, compact)
    -d, --debug                    Enable debug output
        --init-config              Generate sample config file
```

## Configuration

fogwatch can be configured via:
1. Command line arguments
2. Environment variables
3. Configuration file (fogwatch.toml)

### Configuration File Locations
- Project-specific: `./fogwatch.toml`
- User-specific: `~/.config/fogwatch/config.toml`

Generate a sample config:
```bash
fogwatch --init-config
```

## Architecture

fogwatch is built with a modular architecture:

### Core Components
- **TUI Module**: Handles the terminal interface using ratatui
- **WebSocket Client**: Manages real-time log streaming
- **Filter Engine**: Processes and filters log entries
- **State Management**: Maintains application state and log history

### Data Flow
1. User selects a Worker
2. fogwatch creates a tail session via Cloudflare API
3. WebSocket connection established for log streaming
4. Logs processed through filter engine
5. UI updated in real-time with filtered logs

## Contributing

Contributions are welcome! Here are some areas where you can help:

### Planned Features
1. **Enhanced Filtering**:
   - Path pattern matching
   - Response time filters
   - Log level filters
   - Exception type filters

2. **UI Improvements**:
   - Help panel
   - Multiple view modes
   - Search functionality
   - Collapsible JSON

3. **Analytics**:
   - Request rate metrics
   - Response time statistics
   - Error rate tracking

4. **Quality of Life**:
   - Auto-reconnect
   - Session persistence
   - Clipboard integration

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT License - See [LICENSE](LICENSE) for details 
