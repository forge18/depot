# Depot Templates

Depot templates allow you to quickly scaffold new projects with pre-configured structures and files.

## Using Templates

### List Available Templates

```bash
depot template list
```

Search for templates:

```bash
depot template list --search <query>
```

### Initialize Project with Template

```bash
# Interactive mode (select template in wizard)
depot init

# Direct template usage
depot init --template <template-name>

# Non-interactive with template
depot init --template <template-name> --yes
```

## Available Templates

### basic-lua
Basic Lua project template with standard structure.

**Structure:**
- `src/main.lua` - Main entry point
- `lib/` - Library code
- `tests/` - Test files
- `README.md` - Project documentation

### love2d
Love2D game development template.

**Structure:**
- `main.lua` - Main game loop
- `conf.lua` - Love2D configuration
- `src/` - Game source code
- `assets/` - Game assets

### neovim-plugin
Neovim plugin template with Lua plugin structure.

**Structure:**
- `lua/<project_name>/init.lua` - Main plugin entry point
- `plugin/` - VimL plugin files (optional)
- `doc/` - Plugin documentation (optional)

### lapis-web
OpenResty/Lapis web application template.

**Structure:**
- `app.lua` - Main Lapis application
- `views/` - ETLua templates
- `static/` - Static files
- `nginx.conf` - OpenResty/Nginx configuration

### cli-tool
CLI tool template with argument parsing.

**Structure:**
- `src/main.lua` - Main CLI entry point
- `src/` - CLI tool source code
- `lib/` - Library code

## Creating Custom Templates

### From Existing Project

```bash
# Navigate to your project root
cd my-project

# Create template from current project
depot template create my-template --description "My custom template"
```

This will:
1. Copy all project files (excluding `lua_modules/`, `.git/`, etc.)
2. Create a `template.yaml` file in your user templates directory
3. Allow you to customize template variables

### Template Directory Structure

A template directory should contain:

```
template-name/
├── template.yaml      # Template metadata (required)
├── file1.lua          # Template files
├── file2.md
└── subdir/
    └── file3.lua
```

### Template Format

The `template.yaml` file defines template metadata and variables:

```yaml
name: template-name
description: Template description
author: Optional author name
version: Optional template version
variables:
  - name: project_name
    description: Project name
    required: true
  - name: project_version
    description: Project version
    default: "1.0.0"
    required: false
  - name: lua_version
    description: Lua version
    default: "5.4"
    required: false
```

### Variable Substitution

Variables are substituted in template files using `{{variable_name}}` syntax:

```lua
-- Example: src/main.lua
-- {{project_name}}
-- Version: {{project_version}}

print("Hello from {{project_name}}!")
```

When the template is rendered, `{{project_name}}` will be replaced with the actual project name.

### Template Locations

Templates are discovered from two locations:

1. **Built-in templates**: Included with Depot installation
   - Development: `src/templates/` in the Depot source
   - Production: Embedded in the binary or installed separately

2. **User templates**: Custom templates created by users
   - macOS: `~/Library/Application Support/lpm/templates/`
   - Linux: `~/.config/lpm/templates/`
   - Windows: `%APPDATA%\lpm\templates\`

User templates take priority over built-in templates with the same name.

## Template Variables

### Standard Variables

These variables are automatically provided by Depot:

- `project_name` - The project name (required)
- `project_version` - The project version (default: "1.0.0")
- `lua_version` - The Lua version (default: "5.4")

### Custom Variables

You can define additional variables in `template.yaml`:

```yaml
variables:
  - name: author_name
    description: Author name
    required: false
    default: "Developer"
  - name: license
    description: License type
    required: true
```

Custom variables can be provided during template rendering, or use defaults if specified.

## Best Practices

1. **Use descriptive names**: Template names should clearly indicate their purpose
2. **Include README**: Always include a README.md explaining the template structure
3. **Document variables**: Clearly document all template variables in `template.yaml`
4. **Provide defaults**: Use default values for optional variables when possible
5. **Test templates**: Test your templates before sharing them
6. **Version control**: Keep templates in version control for easy updates

## Examples

### Creating a Simple Template

1. Create a directory structure:
```bash
mkdir -p my-template
cd my-template
```

2. Create `template.yaml`:
```yaml
name: my-template
description: My custom template
variables:
  - name: project_name
    required: true
```

3. Create template files:
```bash
echo '-- {{project_name}}' > main.lua
```

4. Use the template:
```bash
depot init --template my-template
```

### Advanced Template with Custom Variables

```yaml
name: advanced-template
description: Advanced template with custom variables
variables:
  - name: project_name
    required: true
  - name: author
    description: Author name
    default: "Unknown"
  - name: license
    description: License type
    default: "MIT"
```

## Troubleshooting

### Template Not Found

If a template is not found:
1. Check template name spelling
2. Verify template exists: `depot template list`
3. Check template location (user vs built-in)
4. Ensure `template.yaml` exists in template directory

### Variable Not Substituted

If variables aren't being substituted:
1. Check variable name spelling (case-sensitive)
2. Ensure variable is defined in `template.yaml`
3. Use `{{variable_name}}` syntax (double curly braces)
4. Check for typos in template files

### Template Creation Fails

If `depot template create` fails:
1. Ensure you're in a project root directory
2. Check that `package.yaml` exists
3. Verify write permissions to user templates directory
4. Check for conflicting template names

