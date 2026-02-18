use std::process::Command;

fn main() {
    println!("🪟 Cargo Wrapper: Launching Windows Bundler...");

    #[cfg(target_os = "linux")]
    {
        eprintln!("❌ Warning: You are running Windows bundler on Linux. This may fail if cross-compilation tools (like cargo-bundle or WiX) aren't set up.");
    }

    let status = Command::new("sh")
        .arg("scripts/bundle_windows.sh")
        .status()
        .expect("Failed to execute bundle script");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
