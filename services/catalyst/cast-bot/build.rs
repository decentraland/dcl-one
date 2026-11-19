//! libwebrtc's Objective-C categories are dropped by the macOS linker unless
//! every object is loaded (`-ObjC`), as the Explorer's `.cargo/config.toml` does.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-ObjC");
    }
}
