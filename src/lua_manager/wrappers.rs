use crate::core::{DepotError, DepotResult};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct WrapperGenerator {
    bin_dir: std::path::PathBuf,
}

impl WrapperGenerator {
    pub fn new(lpm_home: &Path) -> Self {
        Self {
            bin_dir: lpm_home.join("bin"),
        }
    }

    /// Generate binary wrappers for lua and luac
    pub fn generate(&self) -> DepotResult<()> {
        fs::create_dir_all(&self.bin_dir)?;

        self.compile_wrapper("lua")?;
        self.compile_wrapper("luac")?;

        println!("✓ Generated wrappers in {}", self.bin_dir.display());
        self.print_setup_instructions();

        Ok(())
    }

    /// Compile a wrapper binary
    ///
    /// The wrapper is a small Rust program that:
    /// 1. Checks for .lua-version file in current/parent directories
    /// 2. Falls back to current version
    /// 3. Executes the correct binary
    fn compile_wrapper(&self, binary: &str) -> DepotResult<()> {
        let wrapper_source = self.wrapper_source_code(binary);
        let source_path = self.bin_dir.join(format!("{}_wrapper.rs", binary));
        fs::write(&source_path, wrapper_source)?;

        let output_name = if cfg!(target_os = "windows") {
            format!("{}.exe", binary)
        } else {
            binary.to_string()
        };

        // Compile using rustc (or cargo if available)
        let status = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(self.bin_dir.join(&output_name))
            .status()?;

        if !status.success() {
            return Err(DepotError::Package(format!(
                "Failed to compile {} wrapper. Make sure rustc is available.",
                binary
            )));
        }

        // Clean up source file
        fs::remove_file(&source_path)?;

        Ok(())
    }

    fn wrapper_source_code(&self, binary: &str) -> String {
        format!(
            r#"fn main() {{
    use std::env;
    use std::path::PathBuf;
    use std::process::Command;
    
    let lpm_home = env::var("Depot_LUA_DIR")
        .unwrap_or_else(|_| {{
            #[cfg(unix)]
            {{
                let home = env::var("HOME")
                    .or_else(|_| env::var("USERPROFILE"))
                    .unwrap_or_else(|_| {{
                        eprintln!("Error: Could not determine home directory");
                        eprintln!("Neither HOME nor USERPROFILE environment variables are set");
                        std::process::exit(1);
                    }});
                format!("{{}}/.depot", home)
            }}
            #[cfg(windows)]
            {{
                let home = env::var("APPDATA")
                    .or_else(|_| env::var("USERPROFILE").map(|p| format!("{{}}\\\\AppData\\\\Roaming", p)))
                    .unwrap_or_else(|_| {{
                        eprintln!("Error: Could not determine application data directory");
                        eprintln!("Neither APPDATA nor USERPROFILE environment variables are set");
                        std::process::exit(1);
                    }});
                format!("{{}}\\\\depot", home)
            }}
        }});
    
    // Check for .lua-version file
    let mut dir = match env::current_dir() {{
        Ok(d) => d,
        Err(e) => {{
            eprintln!("Error: Failed to get current directory: {{}}", e);
            std::process::exit(1);
        }}
    }};
    let mut version = None;
    
    loop {{
        let version_file = dir.join(".lua-version");
        if version_file.exists() {{
            if let Ok(content) = std::fs::read_to_string(&version_file) {{
                version = Some(content.trim().to_string());
                break;
            }}
        }}
        
        if let Some(parent) = dir.parent() {{
            dir = parent.to_path_buf();
        }} else {{
            break;
        }}
    }}
    
    // Determine which binary to use
    let bin_path = if let Some(ver) = version {{
        let version_dir = PathBuf::from(&lpm_home).join("versions").join(&ver);
        version_dir.join("bin").join("{}")
    }} else {{
        // Use current version (read from current file)
        let current_file = PathBuf::from(&lpm_home).join("current");
        if current_file.exists() {{
            if let Ok(version) = std::fs::read_to_string(&current_file) {{
                let version = version.trim();
                if !version.is_empty() {{
                    PathBuf::from(&lpm_home).join("versions").join(version).join("bin").join("{}")
                }} else {{
                    eprintln!("Error: No Lua version is currently selected");
                    eprintln!("Run: depot lua use <version>");
                    std::process::exit(1);
                }}
            }} else {{
                eprintln!("Error: Failed to read current version file");
                std::process::exit(1);
            }}
        }} else {{
            eprintln!("Error: No Lua version is currently selected");
            eprintln!("Run: depot lua use <version>");
            std::process::exit(1);
        }}
    }};
    
    if !bin_path.exists() {{
        eprintln!("Error: Lua binary not found at {{}}", bin_path.display());
        std::process::exit(1);
    }}
    
    // Execute the binary with all arguments
    let args: Vec<String> = env::args().skip(1).collect();
    let status = match Command::new(&bin_path)
        .args(&args)
        .status() {{
        Ok(s) => s,
        Err(e) => {{
            eprintln!("Error: Failed to execute Lua binary: {{}}", e);
            std::process::exit(1);
        }}
    }};

    std::process::exit(status.code().unwrap_or(1));
}}
"#,
            binary, binary
        )
    }

    fn print_setup_instructions(&self) {
        println!();
        println!("To use Depot-managed Lua versions, add this to your PATH:");
        println!("  {}", self.bin_dir.display());
        println!();

        #[cfg(windows)]
        {
            println!("On Windows, you can add it permanently with:");
            println!("  setx PATH \"%PATH%;{}\"", self.bin_dir.display());
        }

        #[cfg(unix)]
        {
            println!("On Unix/macOS, add to your shell profile:");
            println!("  export PATH=\"{}$PATH\"", self.bin_dir.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_wrapper_generator_new() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        assert!(generator.bin_dir.ends_with("bin"));
    }

    #[test]
    fn test_wrapper_generator_bin_dir_path() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let expected = temp.path().join("bin");
        assert_eq!(generator.bin_dir, expected);
    }

    #[test]
    fn test_wrapper_source_code_lua() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Verify the source code contains expected patterns
        assert!(source.contains("fn main()"));
        assert!(source.contains("Depot_LUA_DIR"));
        assert!(source.contains(".lua-version"));
        assert!(source.contains("lua"));
    }

    #[test]
    fn test_wrapper_source_code_luac() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("luac");

        // Verify the source code contains expected patterns
        assert!(source.contains("fn main()"));
        assert!(source.contains("luac"));
    }

    #[test]
    fn test_wrapper_source_code_contains_error_handling() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Verify error handling is present
        assert!(source.contains("eprintln!"));
        assert!(source.contains("std::process::exit"));
    }

    #[test]
    fn test_print_setup_instructions() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        // Just verify this doesn't panic
        generator.print_setup_instructions();
    }

    #[test]
    fn test_wrapper_source_code_contains_version_check() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should check for .lua-version file
        assert!(source.contains(".lua-version"));
        assert!(source.contains("version_file"));
    }

    #[test]
    fn test_wrapper_source_code_handles_current_version() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should handle current version file
        assert!(source.contains("current"));
        assert!(source.contains("versions"));
    }

    #[test]
    fn test_wrapper_source_code_has_proper_rust_syntax() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Verify basic Rust syntax elements
        assert!(source.contains("fn main() {"));
        assert!(source.contains("use std::"));
        assert!(source.starts_with("fn main()"));
    }

    #[test]
    fn test_wrapper_source_code_uses_environment_vars() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should use environment variables
        assert!(source.contains("env::var"));
        assert!(source.contains("HOME") || source.contains("APPDATA"));
    }

    #[test]
    fn test_wrapper_source_code_has_path_logic() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should traverse parent directories
        assert!(source.contains("parent()"));
        assert!(source.contains("current_dir"));
    }

    #[test]
    fn test_wrapper_source_code_executes_binary() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should execute the actual binary
        assert!(source.contains("Command::new"));
        assert!(source.contains(".args"));
        assert!(source.contains(".status"));
    }

    #[test]
    fn test_wrapper_source_code_different_for_lua_and_luac() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());

        let lua_source = generator.wrapper_source_code("lua");
        let luac_source = generator.wrapper_source_code("luac");

        // Sources should be different (contain different binary names)
        assert_ne!(lua_source, luac_source);
        assert!(lua_source.contains("lua"));
        assert!(!lua_source.contains("luac"));
        assert!(luac_source.contains("luac"));
    }

    #[test]
    fn test_generate_creates_bin_dir() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());

        // Bin dir should not exist initially
        assert!(!generator.bin_dir.exists());

        // Note: We can't test the actual generate() function without rustc
        // but we can test that the bin_dir path is correct
        assert!(generator.bin_dir.ends_with("bin"));
    }

    #[test]
    fn test_wrapper_source_code_handles_windows_paths() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should have Windows-specific path handling
        assert!(source.contains("APPDATA"));
        assert!(source.contains("#[cfg(windows)]"));
    }

    #[test]
    fn test_wrapper_source_code_handles_unix_paths() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should have Unix-specific path handling
        assert!(source.contains("HOME"));
        assert!(source.contains("#[cfg(unix)]"));
    }

    #[test]
    fn test_wrapper_generator_bin_dir_is_absolute() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());

        // If temp.path() is absolute, bin_dir should be too
        if temp.path().is_absolute() {
            assert!(generator.bin_dir.is_absolute());
        }
    }

    #[test]
    fn test_wrapper_source_code_checks_file_existence() {
        let temp = TempDir::new().unwrap();
        let generator = WrapperGenerator::new(temp.path());
        let source = generator.wrapper_source_code("lua");

        // Should check if files exist
        assert!(source.contains(".exists()"));
        assert!(source.contains("version_file.exists()"));
        assert!(source.contains("bin_path.exists()"));
    }
}
