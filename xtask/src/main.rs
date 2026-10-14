//! `cargo xtask layers` — fail when a crate depends on a layer it must not see.
//!
//! The allow-list below IS the architecture. A new workspace crate that is not listed fails the
//! check, so the rule cannot silently fall behind the code (idea borrowed from photocraft).

use std::collections::BTreeMap;
use std::process::ExitCode;

/// crate -> workspace crates it may depend on. External crates are only checked for `egui`/`eframe`.
fn allowed() -> BTreeMap<&'static str, &'static [&'static str]> {
    BTreeMap::from([
        ("arcana-core", &[][..]),
        ("arcana-domain", &["arcana-core"][..]),
        ("arcana-data", &["arcana-core", "arcana-domain"][..]),
        ("arcana-sync", &["arcana-core", "arcana-data"][..]),
        ("arcana-plugin-api", &["arcana-core"][..]),
        (
            "arcana-plugin-host",
            &["arcana-core", "arcana-plugin-api"][..],
        ),
        (
            "arcana-infra",
            &[
                "arcana-core",
                "arcana-domain",
                "arcana-data",
                "arcana-sync",
                "arcana-plugin-api",
                "arcana-plugin-host",
            ][..],
        ),
        ("arcana-ui", &["arcana-core", "arcana-infra"][..]),
        ("arcana-app", &["arcana-ui"][..]),
        ("arcana-plugin-flowchart", &["arcana-plugin-api"][..]),
        ("xtask", &[][..]),
    ])
}

/// Only these crates may pull in the GUI toolkit; everything below the UI layer stays headless.
const GUI_ALLOWED: &[&str] = &["arcana-ui", "arcana-app", "arcana-plugin-flowchart"];
const GUI_CRATES: &[&str] = &["egui", "eframe"];

fn check(meta: &cargo_metadata::Metadata) -> Vec<String> {
    let rules = allowed();
    let members: Vec<_> = meta.workspace_packages();
    let names: Vec<&str> = members.iter().map(|p| p.name.as_str()).collect();
    let mut errors = Vec::new();
    for pkg in &members {
        let name = pkg.name.as_str();
        let Some(ok) = rules.get(name) else {
            errors.push(format!(
                "{name}: not in the layer table (xtask/src/main.rs) — add it"
            ));
            continue;
        };
        for dep in &pkg.dependencies {
            let d = dep.name.as_str();
            if names.contains(&d) && !ok.contains(&d) {
                errors.push(format!("{name} -> {d}: dependency direction not allowed"));
            }
            if GUI_CRATES.contains(&d) && !GUI_ALLOWED.contains(&name) {
                errors.push(format!(
                    "{name} -> {d}: only the UI layer may depend on the GUI toolkit"
                ));
            }
        }
    }
    errors
}

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    if cmd != "layers" {
        eprintln!("usage: cargo xtask layers");
        return ExitCode::from(2);
    }
    let meta = match cargo_metadata::MetadataCommand::new().no_deps().exec() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cargo metadata failed: {e}");
            return ExitCode::from(2);
        }
    };
    let errors = check(&meta);
    if errors.is_empty() {
        println!("layers: OK ({} crates)", meta.workspace_packages().len());
        ExitCode::SUCCESS
    } else {
        for e in &errors {
            eprintln!("layers: {e}");
        }
        ExitCode::FAILURE
    }
}
