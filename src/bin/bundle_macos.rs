use std::process::Command;

fn main() {
    println!("🍎 Cargo Wrapper: Launching macOS Bundler...");
    
    #[cfg(target_os = "linux")]
    {
        eprintln!("❌ Warning: You are running macOS bundler on Linux. This may fail if cross-compilation tools aren't set up.");
    }

    let status = Command::new("sh")
        .arg("scripts/bundle_macos.sh")
        .status()
        .expect("Failed to execute bundle script");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
