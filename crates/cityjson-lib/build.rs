fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_is_windows = matches!(
        std::env::var("CARGO_CFG_TARGET_OS"),
        Ok(target_os) if target_os == "windows"
    );

    if std::env::var_os("CARGO_FEATURE_PROJ").is_some() && target_is_windows {
        println!("cargo:rustc-link-lib=shell32");
    }
}
