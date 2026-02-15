use crate::ui;
use crate::websocket::WebSocketServer;
use depot_core::core::path::{find_project_root, lua_modules_dir};
use depot_core::path_setup::loader::PathSetup;
use depot_core::{DepotError, DepotResult};
use globset::{Glob, GlobMatcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

/// Action to take when a file type changes
#[derive(Debug, Clone)]
pub enum FileAction {
    /// Restart the command
    Restart,
    /// Reload (for WebSocket clients)
    Reload,
    /// No action
    Ignore,
}

/// Configuration for watch mode
pub struct WatchConfig {
    /// Commands to execute (multiple commands run in parallel)
    pub commands: Vec<Vec<String>>,
    /// Paths to watch
    pub paths: Vec<PathBuf>,
    /// Paths to ignore
    pub ignore: Vec<String>,
    /// Clear screen on restart
    pub clear: bool,
    /// Debounce delay in milliseconds
    pub debounce_ms: u64,
    /// Custom file type handlers (extension -> action)
    pub file_handlers: HashMap<String, FileAction>,
    /// WebSocket port (0 = disabled)
    pub websocket_port: u16,
}

impl Default for WatchConfig {
    fn default() -> Self {
        let mut file_handlers = HashMap::new();
        file_handlers.insert("lua".to_string(), FileAction::Restart);
        file_handlers.insert("yaml".to_string(), FileAction::Restart);
        file_handlers.insert("yml".to_string(), FileAction::Restart);
        file_handlers.insert("html".to_string(), FileAction::Reload);
        file_handlers.insert("css".to_string(), FileAction::Reload);
        file_handlers.insert("js".to_string(), FileAction::Reload);

        Self {
            commands: vec![vec!["lua".to_string(), "src/main.lua".to_string()]],
            paths: vec![PathBuf::from("src"), PathBuf::from("lib")],
            ignore: vec![
                "lua_modules/**".to_string(),
                "**/*_test.lua".to_string(),
                "**/*.swp".to_string(),
                "**/.git/**".to_string(),
            ],
            clear: true,
            debounce_ms: 300,
            file_handlers,
            websocket_port: 0,
        }
    }
}

pub struct DevServer {
    config: WatchConfig,
    processes: Vec<Child>,
    project_root: PathBuf,
    should_stop: Arc<AtomicBool>,
    ignore_matchers: Vec<GlobMatcher>,
    websocket: Option<WebSocketServer>,
}

impl DevServer {
    pub fn new(config: WatchConfig, project_root: PathBuf) -> DepotResult<Self> {
        // Build glob matchers for ignore patterns
        let mut ignore_matchers = Vec::new();
        for pattern in &config.ignore {
            let glob = Glob::new(pattern).map_err(|e| {
                DepotError::Package(format!("Invalid ignore pattern '{}': {}", pattern, e))
            })?;
            ignore_matchers.push(glob.compile_matcher());
        }

        // Start WebSocket server if enabled
        let websocket = if config.websocket_port > 0 {
            let ws = WebSocketServer::new(config.websocket_port);
            // Start server in background
            let ws_clone = WebSocketServer::new(config.websocket_port);
            tokio::spawn(async move {
                if let Err(e) = ws_clone.start().await {
                    eprintln!("WebSocket server error: {}", e);
                }
            });
            Some(ws)
        } else {
            None
        };

        Ok(Self {
            config,
            processes: Vec::new(),
            project_root,
            should_stop: Arc::new(AtomicBool::new(false)),
            ignore_matchers,
            websocket,
        })
    }

    /// Start watching and running the command
    pub fn start(&mut self) -> DepotResult<()> {
        let commands_str = self
            .config
            .commands
            .iter()
            .map(|cmd| cmd.join(" "))
            .collect::<Vec<_>>()
            .join(", ");

        ui::UI::server_start(&self.format_paths(), &commands_str);

        // Set up signal handler for graceful shutdown
        let should_stop = Arc::clone(&self.should_stop);
        ctrlc::set_handler(move || {
            ui::UI::server_stop();
            should_stop.store(true, Ordering::SeqCst);
        })
        .map_err(|e| DepotError::Package(format!("Failed to set signal handler: {}", e)))?;

        // Start initial processes
        self.run_commands()?;

        // Set up file watcher
        let (tx, rx) = channel();
        let mut debouncer = new_debouncer(Duration::from_millis(self.config.debounce_ms), tx)
            .map_err(|e| DepotError::Package(format!("Failed to create file watcher: {}", e)))?;

        // Watch all configured paths (resolve relative to project root)
        for path in &self.config.paths {
            let full_path = if path.is_absolute() {
                path.clone()
            } else {
                self.project_root.join(path)
            };

            if full_path.exists() {
                debouncer
                    .watcher()
                    .watch(&full_path, notify::RecursiveMode::Recursive)
                    .map_err(|e| {
                        DepotError::Package(format!(
                            "Failed to watch path {}: {}",
                            full_path.display(),
                            e
                        ))
                    })?;
            }
        }

        // Listen for file changes
        loop {
            // Check if we should stop
            if self.should_stop.load(Ordering::SeqCst) {
                self.stop_processes();
                if let Some(ref ws) = self.websocket {
                    ws.stop();
                }
                break;
            }

            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(events)) => {
                    if let Some(action) = self.should_reload(&events) {
                        let changed_file = events
                            .first()
                            .map(|e| e.path.display().to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        ui::UI::file_changed(&changed_file);

                        match action {
                            FileAction::Restart => {
                                self.stop_processes();

                                if self.config.clear {
                                    ui::UI::clear();
                                }

                                ui::UI::restarting();
                                self.run_commands()?;
                            }
                            FileAction::Reload => {
                                if let Some(ref ws) = self.websocket {
                                    ws.reload();
                                    ui::UI::info("Sent reload signal to browser");
                                } else {
                                    ui::UI::warning("WebSocket not enabled, cannot reload browser");
                                }
                            }
                            FileAction::Ignore => {}
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Watch error: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Continue loop to check should_stop
                    continue;
                }
                Err(e) => {
                    eprintln!("Channel error: {}", e);
                }
            }
        }

        Ok(())
    }

    fn run_commands(&mut self) -> DepotResult<()> {
        ui::UI::status("Starting commands...");

        let commands = self.config.commands.clone();
        for command in &commands {
            self.run_single_command(command)?;
        }

        Ok(())
    }

    fn run_single_command(&mut self, command: &[String]) -> DepotResult<()> {
        if command.is_empty() {
            return Err(DepotError::Package("Empty command".to_string()));
        }

        // Use LuaRunner for Lua commands to get proper path setup
        // Check if it's a Lua command that should use LuaRunner
        let is_lua_command =
            command[0] == "lua" || command[0] == "luajit" || command[0].ends_with("lua");

        if is_lua_command {
            // Use LuaRunner for proper Depot integration
            let lua_modules = lua_modules_dir(&self.project_root);
            if !lua_modules.exists() {
                return Err(DepotError::Package(
                    "lua_modules directory not found. Run 'depot install' first.".to_string(),
                ));
            }

            // Ensure loader is installed
            PathSetup::install_loader(&self.project_root)?;

            // Build command with proper paths
            let mut cmd = Command::new(&command[0]);
            cmd.current_dir(&self.project_root);

            // Set up LUA_PATH and LUA_CPATH like LuaRunner does
            let lua_path = format!(
                "{}/?.lua;{}/?/init.lua;{}/?/?.lua;",
                lua_modules.to_string_lossy(),
                lua_modules.to_string_lossy(),
                lua_modules.to_string_lossy()
            );
            cmd.env("LUA_PATH", lua_path);

            let cpath_ext = if cfg!(target_os = "windows") {
                "dll"
            } else if cfg!(target_os = "macos") {
                "dylib"
            } else {
                "so"
            };
            let lua_cpath = format!(
                "{}/?.{};{}/?/init.{};",
                lua_modules.to_string_lossy(),
                cpath_ext,
                lua_modules.to_string_lossy(),
                cpath_ext
            );
            cmd.env("LUA_CPATH", lua_cpath);

            // Add depot.loader require
            let depot_dir = lua_modules.join("depot");
            cmd.arg("-e").arg(format!(
                "package.path = '{}' .. '/?.lua;' .. package.path; require('depot.loader')",
                depot_dir.to_string_lossy()
            ));

            if command.len() > 1 {
                cmd.args(&command[1..]);
            }

            // Spawn with output visible
            cmd.stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit());

            match cmd.spawn() {
                Ok(child) => {
                    self.processes.push(child);
                    Ok(())
                }
                Err(e) => {
                    ui::UI::error(&format!("Failed to start: {}", e));
                    Err(DepotError::Package(format!(
                        "Failed to start command: {}",
                        e
                    )))
                }
            }
        } else {
            // For non-Lua commands, use standard Command
            let mut cmd = Command::new(&command[0]);
            cmd.current_dir(&self.project_root);

            if command.len() > 1 {
                cmd.args(&command[1..]);
            }

            cmd.stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit());

            match cmd.spawn() {
                Ok(child) => {
                    self.processes.push(child);
                    Ok(())
                }
                Err(e) => {
                    ui::UI::error(&format!("Failed to start: {}", e));
                    Err(DepotError::Package(format!(
                        "Failed to start command: {}",
                        e
                    )))
                }
            }
        }
    }

    fn stop_processes(&mut self) {
        for mut child in self.processes.drain(..) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn should_reload(&self, events: &[DebouncedEvent]) -> Option<FileAction> {
        for event in events {
            let path_str = event.path.to_string_lossy();

            // Check if path matches any ignore patterns using glob matchers
            let should_ignore = self
                .ignore_matchers
                .iter()
                .any(|matcher| matcher.is_match(&event.path));

            if should_ignore {
                continue;
            }

            // Get file extension
            if let Some(ext) = event.path.extension().and_then(|e| e.to_str()) {
                // Check custom file handlers
                if let Some(action) = self.config.file_handlers.get(ext) {
                    return Some(action.clone());
                }
            }

            // Default: restart for Lua/YAML files
            if path_str.ends_with(".lua")
                || path_str.ends_with(".yaml")
                || path_str.ends_with(".yml")
            {
                return Some(FileAction::Restart);
            }
        }

        None
    }

    fn format_paths(&self) -> String {
        self.config
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        self.stop_processes();
        if let Some(ref ws) = self.websocket {
            ws.stop();
        }
    }
}

/// Configuration that can be loaded from package.yaml
#[derive(Debug, Default)]
struct ManifestWatchConfig {
    commands: Option<Vec<Vec<String>>>,
    paths: Option<Vec<PathBuf>>,
    ignore: Option<Vec<String>>,
    websocket_port: Option<u16>,
    file_handlers: Option<HashMap<String, String>>,
}

/// Watch configuration from package.yaml
#[derive(Debug, serde::Deserialize)]
struct WatchConfigYaml {
    command: Option<String>,
    commands: Option<Vec<String>>,
    paths: Option<Vec<String>>,
    ignore: Option<Vec<String>>,
    websocket_port: Option<u16>,
    file_handlers: Option<HashMap<String, String>>,
}

fn load_watch_config_from_manifest(project_root: &Path) -> DepotResult<ManifestWatchConfig> {
    // Try to load from package.yaml
    match depot_core::package::manifest::PackageManifest::load(project_root) {
        Ok(_manifest) => {
            // Try to parse watch section from package.yaml directly
            let package_yaml_path = project_root.join("package.yaml");
            if package_yaml_path.exists() {
                let content = std::fs::read_to_string(&package_yaml_path)?;
                let yaml: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
                    DepotError::Package(format!("Failed to parse package.yaml: {}", e))
                })?;

                if let Some(watch_section) = yaml.get("watch") {
                    let watch_config: WatchConfigYaml =
                        serde_yaml::from_value(watch_section.clone()).map_err(|e| {
                            DepotError::Package(format!("Invalid watch config: {}", e))
                        })?;

                    // Parse file handlers
                    let mut file_handlers = HashMap::new();
                    if let Some(handlers) = watch_config.file_handlers {
                        for (ext, action_str) in handlers {
                            let action = match action_str.as_str() {
                                "restart" => FileAction::Restart,
                                "reload" => FileAction::Reload,
                                "ignore" => FileAction::Ignore,
                                _ => FileAction::Restart,
                            };
                            file_handlers.insert(ext, action);
                        }
                    }

                    // Parse commands (support both single command and multiple)
                    let commands = if let Some(cmds) = watch_config.commands {
                        Some(
                            cmds.iter()
                                .map(|c| c.split_whitespace().map(|s| s.to_string()).collect())
                                .collect(),
                        )
                    } else if let Some(cmd) = watch_config.command {
                        Some(vec![cmd
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()])
                    } else {
                        None
                    };

                    return Ok(ManifestWatchConfig {
                        commands,
                        paths: watch_config.paths.map(|p| {
                            p.iter()
                                .map(|s| {
                                    let pb = PathBuf::from(s);
                                    if pb.is_absolute() {
                                        pb
                                    } else {
                                        project_root.join(pb)
                                    }
                                })
                                .collect()
                        }),
                        ignore: watch_config.ignore,
                        websocket_port: watch_config.websocket_port,
                        file_handlers: if file_handlers.is_empty() {
                            None
                        } else {
                            Some(
                                file_handlers
                                    .iter()
                                    .map(|(k, v)| (k.clone(), format!("{:?}", v).to_lowercase()))
                                    .collect(),
                            )
                        },
                    });
                }
            }

            Ok(ManifestWatchConfig::default())
        }
        Err(_) => Ok(ManifestWatchConfig::default()),
    }
}

/// CLI command handler
pub async fn run(
    command: Option<Vec<String>>,
    paths: Option<Vec<String>>,
    ignore: Option<Vec<String>>,
    no_clear: bool, // Note: CLI uses --no-clear, so this is inverted
    script: Option<String>,
    websocket_port: Option<u16>,
) -> DepotResult<()> {
    // Find project root
    let current_dir = std::env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;

    // Load config from package.yaml if available
    let base_config = load_watch_config_from_manifest(&project_root)?;

    // Determine commands
    let final_commands = if let Some(cmd) = command {
        vec![cmd]
    } else if let Some(script_name) = script {
        // Run a script from package.yaml
        vec![vec!["depot".to_string(), "run".to_string(), script_name]]
    } else if let Some(cmds) = base_config.commands {
        cmds
    } else {
        vec![vec!["lua".to_string(), "src/main.lua".to_string()]]
    };

    // Determine paths (resolve relative to project root)
    let final_paths = paths
        .map(|p| {
            p.iter()
                .map(|s| {
                    let pb = PathBuf::from(s);
                    if pb.is_absolute() {
                        pb
                    } else {
                        project_root.join(pb)
                    }
                })
                .collect()
        })
        .or_else(|| {
            base_config.paths.map(|p| {
                p.iter()
                    .map(|pb| {
                        if pb.is_absolute() {
                            pb.clone()
                        } else {
                            project_root.join(pb)
                        }
                    })
                    .collect()
            })
        })
        .unwrap_or_else(|| vec![project_root.join("src"), project_root.join("lib")]);

    // Build file handlers (merge defaults with config)
    let mut file_handlers = WatchConfig::default().file_handlers;
    if let Some(config_handlers) = base_config.file_handlers {
        for (ext, action_str) in config_handlers {
            let action = match action_str.as_str() {
                "restart" => FileAction::Restart,
                "reload" => FileAction::Reload,
                "ignore" => FileAction::Ignore,
                _ => FileAction::Restart,
            };
            file_handlers.insert(ext, action);
        }
    }

    let config = WatchConfig {
        commands: final_commands,
        paths: final_paths,
        ignore: ignore.or(base_config.ignore).unwrap_or_else(|| {
            vec![
                "lua_modules/**".to_string(),
                "**/*.swp".to_string(),
                "**/.git/**".to_string(),
            ]
        }),
        clear: !no_clear, // Invert: --no-clear means clear=false
        debounce_ms: 300,
        file_handlers,
        websocket_port: websocket_port.unwrap_or(base_config.websocket_port.unwrap_or(0)),
    };

    let mut server = DevServer::new(config, project_root)?;
    server.start()
}
