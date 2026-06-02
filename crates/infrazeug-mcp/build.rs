//! Embeds `generated/api-docs.json` when present; otherwise writes a minimal stub.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated = manifest_dir.join("generated/api-docs.json");
    let out_dir_env = env::var("OUT_DIR").expect("OUT_DIR");
    let out_dir = Path::new(&out_dir_env);
    let embedded = out_dir.join("api-docs.json");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=generated/api-docs.json");

    let bytes = if generated.is_file() {
        fs::read(&generated).expect("read generated/api-docs.json")
    } else {
        br#"{"version":"0","rustdoc_version":null,"crates":[],"items":[]}"#.to_vec()
    };
    fs::write(&embedded, bytes).expect("write OUT_DIR/api-docs.json");
}
