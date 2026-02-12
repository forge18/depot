// Re-export from lpm-core for backward compatibility
pub use depot_core::path_setup::*;

// Keep module structure for imports like crate::path_setup::loader
pub mod loader {
    pub use depot_core::path_setup::loader::*;
}

pub mod runner {
    pub use depot_core::path_setup::runner::*;
}

