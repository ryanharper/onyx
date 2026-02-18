#!/bin/bash
set -e

# Configuration
APP_ID="com.onyx.yt-frontend"
VERSION="25.08"
REPO_DIR="repo"
BUILD_DIR="build-dir"
OUTPUT_BUNDLE="onyx.flatpak"

echo "🐧 Packaging for Linux (Flatpak)..."

# 1. Check Requirements
if ! command -v flatpak &> /dev/null; then
    echo "❌ Error: 'flatpak' command is not found."
    echo "   Please install Flatpak on your host system: https://flatpak.org/setup/"
    exit 1
fi

if ! command -v flatpak-builder &> /dev/null; then
    echo "❌ Error: 'flatpak-builder' is not installed."
    echo "   It should be provided by nix-shell, or install it on your host."
    exit 1
fi

# 2. Add Remote (if needed)
echo "🔍 Checking Flatpak remotes..."
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo

# 3. Ensure Runtime/SDK are installed
echo "📦 Ensuring SDK/Runtime are installed..."
flatpak install --user -y flathub org.freedesktop.Platform//$VERSION org.freedesktop.Sdk//$VERSION org.freedesktop.Sdk.Extension.rust-stable//$VERSION

# 4. Build
echo "🏗️ Building Application..."
# Using --force-clean to ensure a fresh build
# Using --user --install to make it runnable immediately after build for testing
flatpak-builder --user --install --force-clean --repo=$REPO_DIR $BUILD_DIR $APP_ID.yml

# 5. Bundle into single file
echo "🎁 Creating .flatpak bundle..."
flatpak build-bundle $REPO_DIR $OUTPUT_BUNDLE $APP_ID

echo "✅ Build Complete!"
echo "   - Installed to user flatpak repo"
echo "   - Single-file package created: $OUTPUT_BUNDLE (Distributable)"
echo ""
echo "To run installed version:"
echo "   flatpak run $APP_ID"
echo ""
echo "To install the bundle file elsewhere:"
echo "   flatpak install $OUTPUT_BUNDLE"
