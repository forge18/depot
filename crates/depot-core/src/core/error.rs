use thiserror::Error;

pub type DepotResult<T> = Result<T, DepotError>;

#[derive(Error, Debug)]
pub enum DepotError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Package error: {0}")]
    Package(String),

    #[error("Version error: {0}")]
    Version(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Lua error: {0}")]
    Lua(String),

    #[error("WalkDir error: {0}")]
    WalkDir(#[from] walkdir::Error),

    /// A subprocess exited with a non-zero status code.
    /// The exit code should be propagated to the shell.
    #[error("Command exited with code {0}")]
    SubprocessExit(i32),

    /// Security audit found critical or high severity vulnerabilities.
    /// Should exit with code 1.
    #[error("Security audit failed: {0}")]
    AuditFailed(String),
}
