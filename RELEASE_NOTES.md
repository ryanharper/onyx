# Onyx Downloader 0.1.1-alpha

We are excited to announce the pre-release of Onyx Downloader version 0.1.1-alpha! This version brings significant improvements to cross-platform compatibility, specifically targeting Windows users.

## 🚀 What's New

### 🪟 Windows Support
- **Full Windows Compatibility**: Onyx Downloader now supports Windows builds and runtime.
- **Improved Bundling**: Added support for creating MSI installers on Windows using the WiX toolset.
- **Path Handling**: Enhanced cross-platform path handling for external dependencies.

### ✨ Key Features
- **Modern UI**: A sleek and responsive user interface built with the Iced framework.
- **High-Quality Downloads**: Supports 4K/8K video downloads and high-bitrate audio extraction.
- **Built-in Video Player**: Preview and trim videos before downloading with an integrated player.
- **Batch Queue**: Efficiently manage and download multiple videos simultaneously.
- **Self-Managed Dependencies**: Automatically downloads and manages `ffmpeg` and `yt-dlp` for a zero-install experience.

## 📦 Artifacts
The following artifacts have been built and are included in the `artifacts/` directory:

- **Linux (Debian Package)**: `artifacts/yt-frontend_0.1.1-alpha_amd64.deb`

### Missing Platform Artifacts
Due to environment limitations, macOS and Windows artifacts were not pre-built in this environment. However, they can be generated using the following commands on their respective systems:
- **macOS**: `cargo bundle-mac` (Generates `dist/Onyx.dmg`)
- **Windows**: `cargo bundle-windows` (Generates `target/release/bundle/msi/OnyxDownloader.msi`)

## 🛠️ How to Build
To build the application for your specific platform, ensure you have the required system dependencies and run the corresponding cargo alias:

- **Linux**: `cargo bundle-linux`
- **macOS**: `cargo bundle-mac`
- **Windows**: `cargo bundle-windows`

Refer to `README.md` for detailed build requirements and instructions.
