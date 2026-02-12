use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::package::manifest::PackageManifest;
use depot::publish::publisher::Publisher;
use std::env;

pub async fn run(with_binaries: bool) -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;
    let manifest = PackageManifest::load(&project_root)?;

    println!(
        "Publishing {}@{} to LuaRocks...",
        manifest.name, manifest.version
    );

    let publisher = Publisher::new(&project_root, manifest);
    publisher.publish(with_binaries).await?;

    Ok(())
}
