use crate::core::{DepotError, DepotResult};
use crate::lua_manager::versions;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LuaDownloader {
    client: Client,
    cache_dir: PathBuf,
    default_source_url: String,
    version_sources: HashMap<String, String>,
}

impl LuaDownloader {
    pub fn new(cache_dir: PathBuf) -> DepotResult<Self> {
        // Get source URLs from config
        let config = crate::config::Config::load().unwrap_or_default();
        let default_source_url = config.lua_binary_source_url.unwrap_or_else(|| {
            "https://github.com/dyne/luabinaries/releases/latest/download".to_string()
        });
        let version_sources = config.lua_binary_sources.clone().unwrap_or_default();

        Ok(Self {
            client: Client::new(),
            cache_dir,
            default_source_url,
            version_sources,
        })
    }

    /// Get the source URL for a specific version
    fn get_source_url(&self, version: &str) -> &str {
        self.version_sources
            .get(version)
            .map(|s| s.as_str())
            .unwrap_or(&self.default_source_url)
    }

    /// Get the binary filename for current platform
    ///
    /// Extracts major.minor from version (e.g., "5.4.8" -> "54")
    /// This allows future versions to work without code changes
    /// Format: lua<version_code>-<platform> or luac<version_code>-<platform>
    fn get_binary_name(&self, version: &str, binary: &str) -> DepotResult<String> {
        // Use version_code helper from versions module
        let version_code = versions::version_code(version)?;

        // Binary name prefix (lua or luac)
        let prefix = binary; // "lua" or "luac"

        #[cfg(target_os = "windows")]
        {
            Ok(format!("{}{}.exe", prefix, version_code))
        }

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Ok(format!("{}{}", prefix, version_code))
        }

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            Ok(format!("{}{}-linux-arm64", prefix, version_code))
        }

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            Ok(format!("{}{}-macos-x64", prefix, version_code))
        }

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Ok(format!("{}{}-macos-arm64", prefix, version_code))
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(DepotError::Package(format!(
                "Platform not supported: {}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )))
        }
    }

    /// Download Lua binary from configured source
    pub async fn download_binary(&self, version: &str, binary: &str) -> DepotResult<PathBuf> {
        let filename = self.get_binary_name(version, binary)?;
        let source_url = self.get_source_url(version);
        let url = format!("{}/{}", source_url, filename);

        let dest_path = self.cache_dir.join(&filename);

        // Return cached if exists
        if dest_path.exists() {
            return Ok(dest_path);
        }

        println!("Downloading {} for Lua {}...", binary, version);

        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(DepotError::Package(format!(
                "Lua {} is not available for this platform from source: {}\n\
                 Available versions: 5.1.5, 5.3.6, 5.4.8\n\
                 You can set a version-specific source with: depot config set lua_binary_sources.{} <url>",
                version,
                source_url,
                version
            )));
        }

        let bytes = response.bytes().await?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::write(&dest_path, &bytes)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_path, perms)?;
        }

        println!("✓ Downloaded {}", binary);
        Ok(dest_path)
    }

    /// List available versions (only those with pre-built binaries)
    ///
    /// Note: This is a curated list of versions known to have binaries.
    /// Users can still install other versions if they configure a custom source.
    pub fn list_available_versions(&self) -> Vec<String> {
        vec![
            "5.1.5".to_string(),
            "5.3.6".to_string(),
            "5.4.8".to_string(),
        ]
    }

    /// Resolve version aliases to actual versions
    ///
    /// Handles:
    /// - "latest" -> most recent known version
    /// - "5.1", "5.3", "5.4" -> latest patch for that minor version
    /// - Specific versions pass through unchanged
    pub fn resolve_version(&self, version: &str) -> String {
        match version {
            "latest" => "5.4.8".to_string(),
            "5.1" => "5.1.5".to_string(),
            "5.3" => "5.3.6".to_string(),
            "5.4" => "5.4.8".to_string(),
            _ => version.to_string(), // Pass through - might be a future version
        }
    }

    /// Check if a version is in the known list
    ///
    /// This is informational only - installation will still attempt
    /// even for unknown versions (they might be available from custom sources)
    pub fn is_known_version(&self, version: &str) -> bool {
        self.list_available_versions()
            .contains(&version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lua_downloader_list_available_versions() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();
        let versions = downloader.list_available_versions();
        assert!(versions.contains(&"5.1.5".to_string()));
        assert!(versions.contains(&"5.3.6".to_string()));
        assert!(versions.contains(&"5.4.8".to_string()));
    }

    #[test]
    fn test_lua_downloader_resolve_version() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        assert_eq!(downloader.resolve_version("latest"), "5.4.8");
        assert_eq!(downloader.resolve_version("5.1"), "5.1.5");
        assert_eq!(downloader.resolve_version("5.3"), "5.3.6");
        assert_eq!(downloader.resolve_version("5.4"), "5.4.8");
        assert_eq!(downloader.resolve_version("5.4.8"), "5.4.8");
    }

    #[test]
    fn test_lua_downloader_is_known_version() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        assert!(downloader.is_known_version("5.1.5"));
        assert!(downloader.is_known_version("5.3.6"));
        assert!(downloader.is_known_version("5.4.8"));
        assert!(!downloader.is_known_version("5.5.0"));
    }

    #[test]
    fn test_lua_downloader_get_source_url() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();
        let url = downloader.get_source_url("5.4.8");
        assert!(!url.is_empty());
    }

    #[test]
    fn test_lua_downloader_new() {
        let temp = TempDir::new().unwrap();
        let result = LuaDownloader::new(temp.path().to_path_buf());
        assert!(result.is_ok());
        let downloader = result.unwrap();
        assert_eq!(downloader.cache_dir, temp.path().to_path_buf());
    }

    #[test]
    fn test_lua_downloader_get_binary_name() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        // Test that get_binary_name returns a valid name
        let result = downloader.get_binary_name("5.4.8", "lua");
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(name.contains("54"));
    }

    #[test]
    fn test_lua_downloader_get_binary_name_luac() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        let result = downloader.get_binary_name("5.4.8", "luac");
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(name.contains("luac"));
        assert!(name.contains("54"));
    }

    #[test]
    fn test_lua_downloader_get_binary_name_5_1() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        let result = downloader.get_binary_name("5.1.5", "lua");
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(name.contains("51"));
    }

    #[test]
    fn test_lua_downloader_get_binary_name_5_3() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        let result = downloader.get_binary_name("5.3.6", "lua");
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(name.contains("53"));
    }

    #[test]
    fn test_lua_downloader_resolve_version_unknown() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        // Unknown version should pass through unchanged
        assert_eq!(downloader.resolve_version("5.5.0"), "5.5.0");
        assert_eq!(downloader.resolve_version("6.0.0"), "6.0.0");
    }

    #[test]
    fn test_lua_downloader_list_contains_three_versions() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();
        let versions = downloader.list_available_versions();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_lua_downloader_default_source_url() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        // Should use default source URL when no config exists
        assert!(downloader.default_source_url.contains("github.com"));
    }

    #[test]
    fn test_lua_downloader_get_source_url_default() {
        let temp = TempDir::new().unwrap();
        let downloader = LuaDownloader::new(temp.path().to_path_buf()).unwrap();

        // Should return default for unknown version
        let url = downloader.get_source_url("unknown");
        assert_eq!(url, &downloader.default_source_url);
    }
}
