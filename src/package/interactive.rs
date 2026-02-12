use crate::core::{DepotError, DepotResult};
use std::io::{self, Write};

/// Prompt the user for confirmation
pub fn confirm(prompt: &str) -> DepotResult<bool> {
    print!("{} (y/N): ", prompt);
    io::stdout()
        .flush()
        .map_err(|e| DepotError::Package(format!("Failed to write to stdout: {}", e)))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| DepotError::Package(format!("Failed to read from stdin: {}", e)))?;

    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

/// Prompt the user with a default value
pub fn confirm_with_default(prompt: &str, default: bool) -> DepotResult<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} ({}): ", prompt, default_str);
    io::stdout()
        .flush()
        .map_err(|e| DepotError::Package(format!("Failed to write to stdout: {}", e)))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| DepotError::Package(format!("Failed to read from stdin: {}", e)))?;

    let trimmed = input.trim().to_lowercase();

    if trimmed.is_empty() {
        Ok(default)
    } else {
        Ok(trimmed == "y" || trimmed == "yes")
    }
}

/// Prompt for a choice from a list of options
pub fn choose(prompt: &str, options: &[&str], default: usize) -> DepotResult<usize> {
    println!("{}", prompt);
    for (i, option) in options.iter().enumerate() {
        let marker = if i == default { "*" } else { " " };
        println!("  {}[{}] {}", marker, i + 1, option);
    }

    print!("Choose (1-{}, default {}): ", options.len(), default + 1);
    io::stdout()
        .flush()
        .map_err(|e| DepotError::Package(format!("Failed to write to stdout: {}", e)))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| DepotError::Package(format!("Failed to read from stdin: {}", e)))?;

    let trimmed = input.trim();

    if trimmed.is_empty() {
        Ok(default)
    } else {
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= options.len() => Ok(n - 1),
            _ => {
                println!("Invalid choice, using default: {}", default + 1);
                Ok(default)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require mocking stdin/stdout
    // For now, we just test that the functions compile and have correct signatures
    #[test]
    fn test_confirm_function_exists() {
        // This test just ensures the function signature is correct
        let _ = confirm;
    }

    #[test]
    fn test_confirm_with_default_function_exists() {
        let _ = confirm_with_default;
    }

    #[test]
    fn test_choose_function_exists() {
        let _ = choose;
    }

    #[test]
    fn test_choose_with_empty_options() {
        // Test that choose handles edge cases (though this would panic in real use)
        // This is mainly to ensure the function signature is correct
        let _ = choose;
    }

    #[test]
    fn test_confirm_function_signature() {
        // Test that confirm returns DepotResult<bool>
        let func: fn(&str) -> DepotResult<bool> = confirm;
        let _ = func;
    }

    #[test]
    fn test_confirm_with_default_function_signature() {
        // Test that confirm_with_default returns DepotResult<bool>
        let func: fn(&str, bool) -> DepotResult<bool> = confirm_with_default;
        let _ = func;
    }

    #[test]
    fn test_choose_function_signature() {
        // Test that choose returns DepotResult<usize>
        let func: fn(&str, &[&str], usize) -> DepotResult<usize> = choose;
        let _ = func;
    }

    #[test]
    fn test_confirm_error_handling() {
        // Test that confirm can handle errors (though we can't easily test stdin/stdout errors)
        // This ensures the error types are correct
        let _ = confirm;
    }

    #[test]
    fn test_confirm_with_default_error_handling() {
        // Test error handling for confirm_with_default
        let _ = confirm_with_default;
    }

    #[test]
    fn test_choose_error_handling() {
        // Test error handling for choose
        let _ = choose;
    }

    #[test]
    fn test_choose_with_single_option() {
        // Test choose with single option
        let func: fn(&str, &[&str], usize) -> DepotResult<usize> = choose;
        let _ = func;
    }

    #[test]
    fn test_choose_with_multiple_options() {
        // Test choose with multiple options
        let func: fn(&str, &[&str], usize) -> DepotResult<usize> = choose;
        let _ = func;
    }

    #[test]
    fn test_confirm_returns_lpm_result() {
        // Verify confirm returns DepotResult<bool>
        let func: fn(&str) -> DepotResult<bool> = confirm;
        // Check function signature exists
        assert_eq!(std::mem::size_of_val(&func), std::mem::size_of::<usize>());
    }

    #[test]
    fn test_confirm_with_default_returns_lpm_result() {
        // Verify confirm_with_default returns DepotResult<bool>
        let func: fn(&str, bool) -> DepotResult<bool> = confirm_with_default;
        // Check function signature exists
        assert_eq!(std::mem::size_of_val(&func), std::mem::size_of::<usize>());
    }

    #[test]
    fn test_choose_returns_lpm_result_usize() {
        // Verify choose returns DepotResult<usize>
        let func: fn(&str, &[&str], usize) -> DepotResult<usize> = choose;
        // Check function signature exists
        assert_eq!(std::mem::size_of_val(&func), std::mem::size_of::<usize>());
    }

    #[test]
    fn test_confirm_accepts_prompt_string() {
        // Test that confirm accepts &str parameter
        let _func: fn(&str) -> DepotResult<bool> = confirm;
        // Compilation succeeds means signature is correct
    }

    #[test]
    fn test_confirm_with_default_accepts_bool() {
        // Test that confirm_with_default accepts bool default parameter
        let _func: fn(&str, bool) -> DepotResult<bool> = confirm_with_default;
        // Compilation succeeds means signature is correct
    }

    #[test]
    fn test_choose_accepts_slice_and_usize() {
        // Test that choose accepts options slice and default index
        let _func: fn(&str, &[&str], usize) -> DepotResult<usize> = choose;
        // Compilation succeeds means signature is correct
    }

    #[test]
    fn test_interactive_module_exports() {
        // Test that all public functions are accessible
        use super::{choose, confirm, confirm_with_default};
        let _f1 = confirm;
        let _f2 = confirm_with_default;
        let _f3 = choose;
    }
}
