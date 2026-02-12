pub mod credentials;
pub mod error;
pub mod error_help;
pub mod path;
pub mod version;

pub use credentials::CredentialStore;
pub use error::{DepotError, DepotResult};
pub use error_help::{format_error_with_help, ErrorHelp};
