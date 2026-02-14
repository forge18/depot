use super::TemplateDiscovery;
use clap::Subcommand;
use depot_core::DepotResult;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available templates
    List {
        /// Search query to filter templates
        #[arg(short, long)]
        search: Option<String>,
    },
    /// Create a new template from current directory
    Create {
        /// Template name
        name: String,
        /// Template description
        #[arg(short, long)]
        description: Option<String>,
    },
}

pub fn run(cmd: TemplateCommands) -> DepotResult<()> {
    match cmd {
        TemplateCommands::List { search } => list_templates(search),
        TemplateCommands::Create { name, description } => create_template(name, description),
    }
}

fn list_templates(search_query: Option<String>) -> DepotResult<()> {
    let mut templates = TemplateDiscovery::list_templates()?;

    // Filter by search query if provided
    let has_search = search_query.is_some();
    let search_text = search_query.as_deref();
    if let Some(query) = &search_query {
        let query_lower = query.to_lowercase();
        templates.retain(|t| {
            t.name.to_lowercase().contains(&query_lower)
                || t.description.to_lowercase().contains(&query_lower)
        });
    }

    if templates.is_empty() {
        if has_search {
            if let Some(text) = search_text {
                println!("No templates found matching '{}'.", text);
            }
        } else {
            println!("No templates found.");
        }
        println!("\nTemplates can be:");
        println!("  - Built-in templates (in Depot installation)");
        println!(
            "  - User templates (in {})",
            TemplateDiscovery::user_templates_dir()?.display()
        );
        println!("\nCreate a template with: depot template create <name>");
        println!("Search templates with: depot template list --search <query>");
        return Ok(());
    }

    if has_search {
        println!("Found {} template(s):\n", templates.len());
    } else {
        println!("Available templates:\n");
    }

    for template in templates {
        let source_str = match template.source {
            super::discovery::TemplateSource::Builtin => "(built-in)",
            super::discovery::TemplateSource::User => "(user)",
        };
        println!("  {} {}", template.name, source_str);
        println!("    {}", template.description);
        println!();
    }

    Ok(())
}

fn create_template(name: String, description: Option<String>) -> DepotResult<()> {
    use depot_core::core::path::find_project_root;
    use std::env;
    use std::fs;

    let current_dir = env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;

    // Check if we're in a project
    if project_root != current_dir {
        return Err(depot_core::DepotError::Config(
            "Template creation must be run from the project root directory".to_string(),
        ));
    }

    let user_templates_dir = TemplateDiscovery::user_templates_dir()?;
    let template_dir = user_templates_dir.join(&name);

    if template_dir.exists() {
        return Err(depot_core::DepotError::Config(format!(
            "Template '{}' already exists at {}",
            name,
            template_dir.display()
        )));
    }

    // Create template directory
    fs::create_dir_all(&template_dir)?;

    // Copy project files to template (excluding lua_modules, .git, etc.)
    copy_template_files(&project_root, &template_dir)?;

    // Create template.yaml
    let template_yaml = format!(
        r#"name: {}
description: {}
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
"#,
        name,
        description.unwrap_or_else(|| format!("Template for {}", name))
    );
    fs::write(template_dir.join("template.yaml"), template_yaml)?;

    println!(
        "✓ Created template '{}' at {}",
        name,
        template_dir.display()
    );
    println!("\nTemplate files copied from current project.");
    println!("Edit template.yaml to customize template variables.");

    Ok(())
}

fn copy_template_files(
    source: &std::path::Path,
    target: &std::path::Path,
) -> depot_core::DepotResult<()> {
    use std::fs;
    use walkdir::WalkDir;

    let ignore_patterns = [
        "lua_modules",
        ".git",
        ".depot",
        "target",
        "node_modules",
        ".DS_Store",
        "*.lock",
    ];

    for entry in WalkDir::new(source).into_iter().filter_entry(|e| {
        let path = e.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip ignored directories and files
        !ignore_patterns
            .iter()
            .any(|pattern| name.contains(pattern) || path.to_string_lossy().contains(pattern))
    }) {
        let entry = entry?;
        let source_path = entry.path();
        let relative_path = source_path.strip_prefix(source).map_err(|e| {
            depot_core::DepotError::Path(format!("Failed to get relative path: {}", e))
        })?;
        let target_path = target.join(relative_path);

        if source_path.is_dir() {
            fs::create_dir_all(&target_path)?;
        } else {
            // Skip package.yaml (will be generated from template)
            if relative_path == std::path::Path::new("package.yaml") {
                continue;
            }

            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, &target_path)?;
        }
    }

    Ok(())
}
