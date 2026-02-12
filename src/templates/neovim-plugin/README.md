# {{project_name}}

A Neovim plugin written in Lua.

## Installation

Using Depot:

```bash
depot install
```

Or using your Neovim package manager:

```lua
-- Using packer.nvim
use 'your-username/{{project_name}}'

-- Using lazy.nvim
{ 'your-username/{{project_name}}' }
```

## Usage

```lua
require('{{project_name}}').setup({
    -- Configuration options
})
```

## Project Structure

- `lua/{{project_name}}/` - Plugin Lua code
- `lua/{{project_name}}/init.lua` - Main plugin entry point
- `plugin/` - VimL plugin files (optional)
- `doc/` - Plugin documentation (optional)

## Development

```bash
# Install dependencies
depot install

# Run tests
depot run test
```

