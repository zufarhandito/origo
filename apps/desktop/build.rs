fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // GPUI's unoptimized view construction exceeds the default Windows main
        // thread stack. Reserve 8 MiB; Windows commits stack pages on demand.
        println!("cargo:rustc-link-arg-bin=pdf-studio=/STACK:8388608");
    }
}
