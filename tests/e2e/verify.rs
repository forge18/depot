use super::constants;
use super::*;

#[test]
#[ignore = "requires network access"]
fn test_verify_checksums() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install a package (this should create a lockfile with checksums)
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Verify checksums
    ctx.lpm().arg("verify").assert().success();
}
