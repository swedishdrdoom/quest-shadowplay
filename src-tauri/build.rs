use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Standard Tauri build
    tauri_build::build();

    // Only build Swift on macOS
    #[cfg(target_os = "macos")]
    build_swift();
}

#[cfg(target_os = "macos")]
fn build_swift() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let swift_dir = manifest_dir.join("swift");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=swift/CaptureController.swift");

    // Compile Swift to object file
    let status = Command::new("swiftc")
        .args([
            "-O",
            "-whole-module-optimization",
            "-emit-object",
            "-module-name", "CaptureKit",
            "-parse-as-library",
            "-static",
            "-o", out_dir.join("CaptureKit.o").to_str().unwrap(),
            swift_dir.join("CaptureController.swift").to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run swiftc");

    if !status.success() {
        panic!("Swift compilation failed");
    }

    // Create static library
    let status = Command::new("ar")
        .args([
            "rcs",
            out_dir.join("libCaptureKit.a").to_str().unwrap(),
            out_dir.join("CaptureKit.o").to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run ar");

    if !status.success() {
        panic!("Failed to create static library");
    }

    // Tell cargo where to find the library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=CaptureKit");

    // Link required Apple frameworks
    println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=VideoToolbox");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");

    // Link Swift standard library (required for Swift code)
    let swift_lib_dir = get_swift_lib_dir();
    println!("cargo:rustc-link-search=native={}", swift_lib_dir);
    
    // Add rpath so dyld can find Swift libraries at runtime
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", swift_lib_dir);
    
    // Also add the system Swift libraries path
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}

#[cfg(target_os = "macos")]
fn get_swift_lib_dir() -> String {
    // Get the Swift toolchain library directory
    let output = Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .expect("Failed to find swiftc");

    let swiftc_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let toolchain_dir = PathBuf::from(swiftc_path)
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf();

    let lib_dir = toolchain_dir.join("lib").join("swift").join("macosx");
    lib_dir.to_string_lossy().to_string()
}
