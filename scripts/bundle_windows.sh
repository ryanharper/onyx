#!/bin/bash
set -e

# Configuration
APP_NAME="Onyx"
OUTPUT_DIR="target/release/bundle/msi"

echo "🪟 Bundling Windows Application ($APP_NAME)..."

# 1. Build the app
echo "🏗️  Building release binary..."
cargo build --release

# 2. Bundle using cargo-bundle
# This will use settings from Cargo.toml [package.metadata.bundle]
# Note: For Windows, this generates an MSI package and requires WiX Toolset.
echo "📦 Creating MSI installer..."
if cargo bundle --release --format msi; then
    echo "✅ MSI Bundle created successfully."
    echo "📂 Package location: $OUTPUT_DIR"
else
    echo "⚠️  MSI bundling failed. This usually requires WiX Toolset on the host system."
    echo "   Standalone executable is available at: target/release/yt-frontend.exe"
fi

# 3. Finalize
echo "🚀 Windows distribution process complete."
