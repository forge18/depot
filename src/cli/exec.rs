use depot::core::path::find_project_root;
use depot::core::DepotResult;
use depot::path_setup::{LuaRunner, RunOptions};
use std::env;

pub fn run(command: Vec<String>) -> DepotResult<()> {
    if command.is_empty() {
        return Err(depot::core::DepotError::Package(
            "No command provided".to_string(),
        ));
    }

    let current_dir = env::current_dir()?;
    // Verify we're in a project (for error checking)
    find_project_root(&current_dir)?;

    // Join command parts into a single command string
    let command_str = command.join(" ");

    // Execute command with automatic path setup
    let exit_code = LuaRunner::exec_command(&command_str, RunOptions::default())?;
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_with_empty_command() {
        // Test that exec handles empty command
        let command = vec![];
        let result = run(command);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No command provided"));
    }

    #[test]
    fn test_exec_command_joining() {
        // Test command joining logic
        let command = [
            "lua".to_string(),
            "-e".to_string(),
            "print('hello')".to_string(),
        ];
        let command_str = command.join(" ");
        assert_eq!(command_str, "lua -e print('hello')");
    }
}
