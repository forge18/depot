use lpm::core::credentials::CredentialStore;
use lpm::core::{LpmError, LpmResult};
use std::io::{self, Write};

pub async fn run() -> LpmResult<()> {
    println!("LuaRocks Login");
    println!("Enter your LuaRocks credentials:");
    println!();

    // Get username
    print!("Username: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    if username.is_empty() {
        return Err(LpmError::Package("Username cannot be empty".to_string()));
    }

    // Get API key
    print!("API Key: ");
    io::stdout().flush()?;
    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err(LpmError::Package("API key cannot be empty".to_string()));
    }

    // Store credentials
    CredentialStore::store("luarocks_username", &username)?;
    CredentialStore::store("luarocks_api_key", &api_key)?;

    println!();
    println!("✓ Credentials stored securely");

    Ok(())
}
