# Getting Started

This guide will help you create your first LPM project and start managing dependencies.

## Initialize a New Project

Create a new directory for your project and initialize it:

```bash
mkdir my-lua-project
cd my-lua-project
lpm init
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
lpm install luasocket
```

This will:
1. Download `luasocket` from LuaRocks
2. Install it to `./lua_modules/`
3. Update `package.yaml` with the dependency
4. Generate `lpm.lock` with exact versions and BLAKE3 checksums

### Install Multiple Packages

You can install multiple packages at once:

```bash
lpm install luasocket penlight lua-cjson
```

### Install with Version Constraints

```bash
lpm install luasocket@3.0.0        # Exact version
lpm install penlight@^1.13.0       # Compatible version (>=1.13.0 <2.0.0)
lpm install lua-cjson@~2.1.0       # Patch version (>=2.1.0 <2.2.0)
```

### Install All Dependencies

If you have a `package.yaml` with dependencies already listed:

```bash
lpm install
```

This installs all dependencies listed in `package.yaml`.

## Using Installed Packages

LPM automatically sets up `package.path` so your Lua code can find installed packages:

```lua
-- main.lua
local socket = require("socket")
local pl = require("pl")

print("Hello from LPM!")
```

Run your code:

```bash
lua main.lua
```

LPM's loader automatically configures `package.path` to include `./lua_modules/`.

## Project Structure

After installing dependencies, your project will look like:

```
my-lua-project/
├── package.yaml          # Your project manifest
├── lpm.lock              # Lockfile (auto-generated)
├── lua_modules/          # Installed dependencies
│   ├── .lpm/            # LPM metadata
│   ├── luasocket/
│   └── penlight/
└── main.lua             # Your code
```

## Lua Version Management

LPM includes a built-in Lua version manager, so you don't need to install Lua separately:

```bash
# Install a Lua version
lpm lua install latest

# Use it globally
lpm lua use 5.4.8

# Or set it for this project
lpm lua local 5.4.8
```

After installing Lua, add `~/.lpm/bin/` to your PATH to use the `lua` and `luac` commands. The wrappers automatically detect `.lua-version` files in your project directories.

## Global Tool Installation

Install development tools globally so they're available everywhere:

```bash
# Install tools globally
lpm install -g luacheck
lpm install -g busted

# Now available everywhere (after adding ~/.lpm/bin/ to PATH)
luacheck my_file.lua
busted
```

Global tools are installed to `~/.lpm/global/` and executables are created in `~/.lpm/bin/`.

## Next Steps

- Learn about [Package Management](Package-Management) for advanced dependency management
- Check out [CLI Commands](CLI-Commands) for all available commands
- Read about [Rust Extensions](Rust-Extensions) if you need native modules
- Review [Security](Security) best practices

