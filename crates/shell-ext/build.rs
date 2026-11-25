fn main() {
    println!("cargo:rerun-if-changed=exports.def");
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let def_path = std::path::Path::new(&manifest_dir).join("exports.def");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_path.display());
    }
}
