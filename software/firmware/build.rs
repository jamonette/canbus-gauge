use std::{env, fs, path::PathBuf};

// Add `memory.x` to the linker search path and add the cortex-m-rt & defmt
// linker scripts
fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    fs::copy("memory.x", out.join("memory.x")).expect("copy memory.x to OUT_DIR");
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
