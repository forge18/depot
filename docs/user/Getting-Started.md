# Getting Started

This guide will help you create your first Depot project and start managing dependencies.

## Initialize a New Project

Create a new directory for your project and initialize it:

```bash
mkdir my-lua-project
cd my-lua-project
depot init
```

This creates a `package.yaml` file in your project directory:

```yaml
name: my-lua-project
version: 0.1.0
lua_version: "5.4"

dependencies: {}

dev_dependencies: {}
```

## Install Dependencies

### Install a Single Package

```bash
depot install luasocket
```

This will:
1. Download `luasocket` from LuaRocks
2. Install it to `./lua_modules/`
3. Update `package.yaml` with the dependency
4. Generate `depot.lock` with exact versions and BLAKE3 checksums

### Install Multiple Packages

You can install multiple packages at once:

```bash
depot install luasocket penlight lua-cjson
```

### Install with Version Constraints

```bash
depot install luasocket@3.0.0        # Exact version
depot install penlight@^1.13.0       # Compatible version (>=1.13.0 <2.0.0)
depot install lua-cjson@~2.1.0       # Patch version (>=2.1.0 <2.2.0)
```

### Install All Dependencies

If you have a `package.yaml` with dependencies already listed:

```bash
depot install
```

This installs all dependencies listed in `package.yaml`.

## Using Installed Packages

Depot automatically sets up `package.path` so your Lua code can find installed packages:

```lua
-- main.lua
local socket = require("socket")
local pl = require("pl")

print("Hello from Depot!")
```

Run your code:

```bash
lua main.lua
```

Depot's loader automatically configures `package.path` to include `./lua_modules/`.

## Project Structure

After installing dependencies, your project will look like:

```
my-lua-project/
├── package.yaml          # Your project manifest
├── depot.lock              # Lockfile (auto-generated)
├── lua_modules/          # Installed dependencies
│   ├── .depot/            # Depot metadata
│   ├── luasocket/
│   └── penlight/
└── main.lua             # Your code
```

## Lua Version Management

Depot includes a built-in Lua version manager, so you don't need to install Lua separately:

```bash
# Install a Lua version
depot lua install latest

# Use it globally
depot lua use 5.4.8

# Or set it for this project
depot lua local 5.4.8
```

After installing Lua, add `~/.depot/bin/` to your PATH to use the `lua` and `luac` commands. The wrappers automatically detect `.lua-version` files in your project directories.

## Global Tool Installation

Install development tools globally so they're available everywhere:

```bash
# Install tools globally
depot install -g luacheck
depot install -g busted

# Now available everywhere (after adding ~/.depot/bin/ to PATH)
luacheck my_file.lua
busted
```

Global tools are installed to `~/.depot/global/` and executables are created in `~/.depot/bin/`.

## Next Steps

- Learn about [Package Management](Package-Management) for advanced dependency management
- Check out [CLI Commands](CLI-Commands) for all available commands
- Read about [Rust Extensions](Rust-Extensions) if you need native modules
- Review [Security](Security) best practices

