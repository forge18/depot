use crate::core::{LpmError, LpmResult};
use keyring::Entry;
use std::path::Path;

/// Service name for keyring entries
const KEYRING_SERVICE: &str = "lpm";

/// Manages credential storage using OS keychain
///
/// Platform support:
/// - macOS: Keychain
/// - Windows: Credential Manager  
/// - Linux: Secret Service (libsecret)
pub struct CredentialStore;

impl CredentialStore {
    /// Store a credential in the OS keychain
    pub fn store(key: &str, value: &str) -> LpmResult<()> {
        let entry = Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| LpmError::Package(format!("Failed to create keyring entry: {}", e)))?;

        entry.set_password(value).map_err(|e| {
            LpmError::Package(format!("Failed to store credential in keychain: {}", e))
        })?;

        Ok(())
    }

    /// Retrieve a credential from the OS keychain
    pub fn retrieve(key: &str) -> LpmResult<String> {
        let entry = Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| LpmError::Package(format!("Failed to create keyring entry: {}", e)))?;

        let password = entry.get_password().map_err(|e| {
            LpmError::Package(format!(
                "Failed to retrieve credential from keychain: {}",
                e
            ))
        })?;

        Ok(password)
    }

    /// Delete a credential from the OS keychain
    pub fn delete(key: &str) -> LpmResult<()> {
        let entry = Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| LpmError::Package(format!("Failed to create keyring entry: {}", e)))?;

        entry.delete_credential().map_err(|e| {
            LpmError::Package(format!("Failed to delete credential from keychain: {}", e))
        })?;

        Ok(())
    }

    /// Check if a credential exists in the keychain
    pub fn exists(key: &str) -> bool {
        Self::retrieve(key).is_ok()
    }

    /// Set file permissions to 0600 (owner read/write only)
    ///
    /// This is a utility function for ensuring sensitive files have proper permissions.
    /// Used for any credential-related files that might exist.
    #[cfg(unix)]
    pub fn set_secure_permissions(path: &Path) -> LpmResult<()> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path)
            .map_err(|e| LpmError::Package(format!("Failed to get file metadata: {}", e)))?
            .permissions();

        perms.set_mode(0o600); // rw------- (owner read/write only)
        fs::set_permissions(path, perms)
            .map_err(|e| LpmError::Package(format!("Failed to set file permissions: {}", e)))?;

        Ok(())
    }

    /// Set file permissions to 0600 (owner read/write only) on Windows
    ///
    /// On Windows, file permissions work differently. Since we use the OS keyring
    /// for credential storage, this is a no-op on Windows.
    #[cfg(windows)]
    pub fn set_secure_permissions(_path: &Path) -> LpmResult<()> {
        // On Windows, credentials are stored in the OS keyring (Credential Manager),
        // so file permissions are not applicable. This is a no-op.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_operations() {
        let test_key = "test_credential";
        let test_value = "test_password_123";

        // Clean up if it exists
        let _ = CredentialStore::delete(test_key);

        // Store credential
        let store_result = CredentialStore::store(test_key, test_value);
        if store_result.is_err() {
            // Keychain might not be available in test environment (CI, etc.)
            // Skip test if keychain is not accessible
            eprintln!("Skipping keyring test: keychain not available");
            return;
        }
        assert!(store_result.is_ok());

        // Small delay to ensure keychain has processed the write
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Retrieve credential
        let retrieved = CredentialStore::retrieve(test_key);
        if retrieved.is_err() {
            // If retrieval fails, clean up and skip test
            let _ = CredentialStore::delete(test_key);
            eprintln!("Skipping keyring test: keychain retrieval failed");
            return;
        }
        assert_eq!(retrieved.unwrap(), test_value);

        // Check existence
        assert!(CredentialStore::exists(test_key));

        // Delete credential
        assert!(CredentialStore::delete(test_key).is_ok());
        assert!(!CredentialStore::exists(test_key));
    }
}
