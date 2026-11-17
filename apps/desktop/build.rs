fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/branding/origo.rc");
    println!("cargo:rerun-if-changed=resources/branding/origo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        if let Err(error) = embed_resource::compile(
            "resources/branding/origo.rc",
            embed_resource::ParamsIncludeDirs(["resources/branding"]),
        )
        .manifest_required()
        {
            panic!("Cannot embed Origo Windows icon/resources: {error}");
        }
        // GPUI's unoptimized view construction exceeds the default Windows main
        // thread stack. Reserve 8 MiB; Windows commits stack pages on demand.
        println!("cargo:rustc-link-arg-bin=pdf-studio=/STACK:8388608");
    }
}
