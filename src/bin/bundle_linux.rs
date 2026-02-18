use std::process::Command;


fn main() {
    // Ensure we are in the project root (where Cargo.toml is)
    // Cargo sets CARGO_MANIFEST_DIR, but running "cargo run" usually sets CWD to root anyway.
    
    println!("🐧 Cargo Wrapper: Launching Flatpak Bundler...");
    
    #[cfg(target_os = "windows")]
    {
        eprintln!("❌ Error: This script is for Linux.");
        std::process::exit(1);
    }

    let status = Command::new("sh")
        .arg("scripts/bundle_flatpak.sh")
        .status()
        .expect("Failed to execute bundle script");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
