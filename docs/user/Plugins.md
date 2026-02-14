# Depot Plugins

Depot supports plugins as separate executables that extend core functionality. Plugins are automatically discovered and can be installed globally.

## Available Plugins

### `depot-watch` - Dev Server / Watch Mode

Auto-reload your Lua applications on file changes. Perfect for Love2D, Neovim plugins, OpenResty, and general development.

#### Installation

```bash
depot install -g depot-watch
```

#### Basic Usage

```bash
# Watch and restart on changes
depot watch

# Alias for watch
depot watch dev

# Don't clear screen on reload
depot watch --no-clear

# Enable WebSocket server for browser reload
depot watch --websocket-port 35729
```

#### Features

- **Multiple commands**: Run multiple commands in parallel
- **Custom file type handlers**: Configure different actions per file extension
- **WebSocket support**: Browser auto-reload for HTML/CSS/JS files
- **Enhanced terminal UI**: Colored output with timestamps and status indicators
- File watching with debouncing
- Automatic process restart
- Configurable ignore patterns
- Screen clearing (optional)
- Works with `depot run` scripts

#### Configuration

Configure `depot-watch` in your `package.yaml`:

```yaml
watch:
  # Single command (legacy support)
  command: "lua src/main.lua"
  
  # Multiple commands (run in parallel)
  commands:
    - "lua src/server.lua"
    - "lua src/worker.lua"
  
  # Paths to watch
  paths:
    - "src"
    - "lib"
    - "assets"
  
  # Patterns to ignore
  ignore:
    - "**/*.test.lua"
    - "**/tmp/**"
    - "**/.git/**"
  
  # WebSocket server port (0 = disabled)
  websocket_port: 35729
  
  # Custom file type handlers
  file_handlers:
    lua: restart      # Restart command on .lua changes
    yaml: restart    # Restart command on .yaml changes
    html: reload     # Send reload signal to browser
    css: reload      # Send reload signal to browser
    js: reload       # Send reload signal to browser
    txt: ignore      # Ignore .txt file changes
  
  # Debounce delay in milliseconds
  debounce_ms: 300
  
  # Clear screen on restart
  clear: true
```

#### File Actions

Configure how different file types are handled:

- **`restart`**: Restart the command(s) when files of this type change
- **`reload`**: Send reload signal via WebSocket (for browser reload)
- **`ignore`**: No action when files of this type change

#### WebSocket Browser Reload

Enable browser auto-reload by setting `websocket_port` in your configuration or using the `--websocket-port` flag:

```bash
depot watch --websocket-port 35729
```

Then add this script to your HTML files:

```html
<script>
  const ws = new WebSocket('ws://localhost:35729');
  ws.onmessage = function(event) {
    const data = JSON.parse(event.data);
    if (data.type === 'reload') {
      location.reload();
    }
  };
</script>
```

When HTML, CSS, or JS files change, the browser will automatically reload.

#### Multiple Commands

Run multiple commands in parallel:

```yaml
watch:
  commands:
    - "lua src/server.lua"
    - "lua src/worker.lua"
    - "lua src/scheduler.lua"
```

All commands will start simultaneously and restart together when watched files change.

#### CLI Options

```bash
depot watch [OPTIONS]

Options:
  -c, --command <COMMAND>    Command to run (can be specified multiple times)
  -p, --paths <PATHS>        Paths to watch (default: src/, lib/)
  -i, --ignore <PATTERNS>    Patterns to ignore
      --no-clear             Don't clear screen on restart
  -s, --script <SCRIPT>      Script name from package.yaml to run
      --websocket-port <PORT>  WebSocket port for browser reload (0 = disabled)
```

## Installing Plugins

Plugins are installed globally and become available as `depot <plugin-name>`:

```bash
# Install a plugin
depot install -g depot-watch

# Plugins are automatically discovered
depot watch --help
```

## Plugin Locations

Plugins are installed to:
- **macOS**: `~/Library/Application Support/lpm/bin/`
- **Linux**: `~/.config/lpm/bin/`
- **Windows**: `%APPDATA%\lpm\bin\`
- **Legacy**: `~/.depot/bin/` (for backwards compatibility)

Plugins can also be installed anywhere in your PATH.

## Managing Plugins

Use the `depot plugin` commands to manage plugins:

```bash
# List installed plugins
depot plugin list

# Show plugin information
depot plugin info <plugin-name>

# Update plugins
depot plugin update
depot plugin update <plugin-name>

# Check for outdated plugins
depot plugin outdated

# Search for plugins
depot plugin search <query>

# Configure plugins
depot plugin config get <plugin> <key>
depot plugin config set <plugin> <key> <value>
depot plugin config show <plugin>
```

## Creating Plugins

See the [Plugin Development Guide](../contributing/Plugin-Development.md) for details on creating your own plugins.

