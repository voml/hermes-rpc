//! End-to-end code generation helper for TypeScript compile verification.
//!
//! Usage: cargo run -p hermes-generator --example generate_e2e \
//!   --fixture <path.to.contract.json> \
//!   --desktop <out_dir> \
//!   --web <out_dir_web>
//!
//! Reads a `vos/hermes.contract/v0` artifact, generates method-level TS files
//! for both AOT targets, and writes each plan into its own directory. The
//! directories can then be type-checked with `tsc --noEmit`.

use std::env;
use std::fs;
use std::path::PathBuf;

use hermes_generator::{AotTarget, FileOwnership, generate_service};
use vos_types::{ContractArtifact, ContractFormat};

fn parse_args() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let args: Vec<String> = env::args().collect();
    let mut fixture: Option<PathBuf> = None;
    let mut desktop: Option<PathBuf> = None;
    let mut web: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                fixture = Some(PathBuf::from(args.get(i + 1).ok_or("--fixture needs a value")?));
                i += 2;
            }
            "--desktop" => {
                desktop = Some(PathBuf::from(args.get(i + 1).ok_or("--desktop needs a value")?));
                i += 2;
            }
            "--web" => {
                web = Some(PathBuf::from(args.get(i + 1).ok_or("--web needs a value")?));
                i += 2;
            }
            other => return Err(format!("unknown arg {other}")),
        }
    }

    Ok((
        fixture.ok_or("missing --fixture")?,
        desktop.ok_or("missing --desktop")?,
        web.ok_or("missing --web")?,
    ))
}

fn ownership_label(o: FileOwnership) -> &'static str {
    match o {
        FileOwnership::Generated => "GEN",
        FileOwnership::Template => "TPL",
    }
}

fn main() {
    let (fixture_path, desktop_dir, web_dir) = parse_args().expect("arguments");

    let json = fs::read_to_string(&fixture_path)
        .unwrap_or_else(|err| panic!("cannot read fixture {}: {err}", fixture_path.display()));
    let parsed: ContractArtifact =
        serde_json::from_str(&json).unwrap_or_else(|err| panic!("fixture JSON decode: {err}"));
    let artifact = ContractArtifact::seal(ContractFormat::Hermes, parsed.producer, parsed.payload);

    // Desktop target
    let desktop_plan = generate_service(&artifact, AotTarget::Desktop)
        .unwrap_or_else(|err| panic!("desktop generation failed: {err}"));
    if desktop_dir.exists() {
        fs::remove_dir_all(&desktop_dir).unwrap_or_else(|err| panic!("clean desktop dir: {err}"));
    }
    let desktop_report = desktop_plan
        .write_to(&desktop_dir)
        .unwrap_or_else(|err| panic!("desktop write_to: {err}"));
    println!(
        "Desktop: wrote {} files to {}",
        desktop_report.written().len(),
        desktop_dir.display()
    );
    for file in desktop_plan.files() {
        println!("  [{}] {}", ownership_label(file.ownership()), file.path());
    }

    // Web target
    let web_plan =
        generate_service(&artifact, AotTarget::Web).unwrap_or_else(|err| panic!("web generation failed: {err}"));
    if web_dir.exists() {
        fs::remove_dir_all(&web_dir).unwrap_or_else(|err| panic!("clean web dir: {err}"));
    }
    let web_report = web_plan
        .write_to(&web_dir)
        .unwrap_or_else(|err| panic!("web write_to: {err}"));
    println!("Web: wrote {} files to {}", web_report.written().len(), web_dir.display());
    for file in web_plan.files() {
        println!("  [{}] {}", ownership_label(file.ownership()), file.path());
    }

    // Diagnostics
    if !desktop_plan.diagnostics().is_empty() {
        println!("Desktop diagnostics:");
        for d in desktop_plan.diagnostics() {
            println!("  - {}", d.message());
        }
    }
    if !web_plan.diagnostics().is_empty() {
        println!("Web diagnostics:");
        for d in web_plan.diagnostics() {
            println!("  - {}", d.message());
        }
    }
}
