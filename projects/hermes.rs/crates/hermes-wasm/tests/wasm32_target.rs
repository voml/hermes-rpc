//! Gate: when a `wasm32-unknown-unknown` release artifact exists, it must be a
//! plausible Hermes WASM binary. The build is not triggered here; run
//! `cargo build -p hermes-wasm --target wasm32-unknown-unknown --release` or
//! `node scripts/build-wasm.mjs` first.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn wasm_artifact() -> PathBuf {
    workspace_root().join("target/wasm32-unknown-unknown/release/hermes_wasm.wasm")
}

#[test]
fn release_wasm32_artifact_is_sane_when_present() {
    let wasm = wasm_artifact();
    if !wasm.is_file() {
        eprintln!("skipping: missing wasm artifact at {}", wasm.display());
        return;
    }
    let bytes = std::fs::metadata(&wasm).expect("stat wasm").len();
    assert!(bytes > 8_192, "wasm artifact suspiciously small: {bytes} bytes");
    assert!(bytes < 2_000_000, "wasm artifact too large for browser target: {bytes} bytes");
}
