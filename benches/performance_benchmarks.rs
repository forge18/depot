//! Performance benchmarks for Depot

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use depot::core::version::{parse_constraint, Version};
use depot::package::manifest::PackageManifest;

fn benchmark_version_parsing(c: &mut Criterion) {
    c.bench_function("parse_version", |b| {
        b.iter(|| {
            Version::parse(black_box("1.2.3")).unwrap();
        })
    });
}

fn benchmark_version_constraint_parsing(c: &mut Criterion) {
    c.bench_function("parse_constraint", |b| {
        b.iter(|| {
            parse_constraint(black_box("^1.2.3")).unwrap();
            parse_constraint(black_box("~1.0.0")).unwrap();
            parse_constraint(black_box(">=1.2.3")).unwrap();
        })
    });
}

fn benchmark_version_satisfaction(c: &mut Criterion) {
    let version = Version::parse("1.2.3").unwrap();
    let constraint = parse_constraint("^1.2.0").unwrap();

    c.bench_function("version_satisfies", |b| {
        b.iter(|| {
            version.satisfies(black_box(&constraint));
        })
    });
}

fn benchmark_manifest_serialization(c: &mut Criterion) {
    let mut manifest = PackageManifest::default("bench-package".to_string());
    manifest.version = "1.0.0".to_string();
    for i in 0..100 {
        manifest
            .dependencies
            .insert(format!("dep-{}", i), format!("^{}.0.0", i % 10));
    }

    c.bench_function("manifest_to_yaml", |b| {
        b.iter(|| {
            serde_yaml::to_string(black_box(&manifest)).unwrap();
        })
    });
}

fn benchmark_manifest_deserialization(c: &mut Criterion) {
    let mut manifest = PackageManifest::default("bench-package".to_string());
    manifest.version = "1.0.0".to_string();
    for i in 0..100 {
        manifest
            .dependencies
            .insert(format!("dep-{}", i), format!("^{}.0.0", i % 10));
    }
    let yaml = serde_yaml::to_string(&manifest).unwrap();

    c.bench_function("manifest_from_yaml", |b| {
        b.iter(|| {
            serde_yaml::from_str::<PackageManifest>(black_box(&yaml)).unwrap();
        })
    });
}

fn benchmark_dependency_resolution(c: &mut Criterion) {
    let mut manifest = PackageManifest::default("test".to_string());
    manifest
        .dependencies
        .insert("luasocket".to_string(), "^3.0.0".to_string());
    manifest
        .dependencies
        .insert("penlight".to_string(), "^1.13.0".to_string());

    c.bench_function("resolve_dependencies", |b| {
        b.iter(|| {
            // Note: This requires network access and may be slow
            // In real benchmarks, we'd mock the LuaRocks client
            // For now, this is a placeholder
            black_box(&manifest);
        })
    });
}

criterion_group!(
    benches,
    benchmark_version_parsing,
    benchmark_version_constraint_parsing,
    benchmark_version_satisfaction,
    benchmark_manifest_serialization,
    benchmark_manifest_deserialization,
    benchmark_dependency_resolution
);
criterion_main!(benches);
