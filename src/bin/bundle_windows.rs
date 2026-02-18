use std::process::Command;

fn main() {
    println!("🪟 Cargo Wrapper: Launching Windows Bundler...");

    #[cfg(target_os = "linux")]
    {
        eprintln!("❌ Warning: You are running Windows bundler on Linux. This may fail if cross-compilation tools (like cargo-bundle or WiX) aren't set up.");
    }

    // 1. Build the app
    println!("🏗️  Building release binary...");
    let mut cargo_build = Command::new("cargo");
    cargo_build.arg("build").arg("--release");

    let status = cargo_build.status().expect("Failed to execute cargo build");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    // 2. Bundle using cargo-bundle
    println!("📦 Creating MSI installer...");
    let mut cargo_bundle = Command::new("cargo");
    cargo_bundle.arg("bundle").arg("--release").arg("--format").arg("msi");

    let status = cargo_bundle.status();

    match status {
        Ok(s) if s.success() => {
            println!("✅ MSI Bundle created successfully.");
            println!("📂 Package location: target/release/bundle/msi");
        }
        _ => {
            println!("⚠️  MSI bundling failed. This usually requires WiX Toolset on the host system.");
            println!("   Standalone executable is available at: target/release/yt-frontend.exe");
        }
    }

    println!("🚀 Windows distribution process complete.");
}
