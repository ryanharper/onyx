# 🚀 CI/CD Guidelines for Onyx

**To:** Jules (DevOps / CI/CD Manager)  
**From:** Development Team  
**Date:** 2026-02-18  

Hi Jules, welcome to the Onyx project! 👋

This document outlines the build system, artifacts, and requirements for setting up our upstream CI/CD pipeline. Our goal is confusing-free automation for Linux (Flatpak) and macOS (DMG) releases.

---

## 🏗️ Build System Overview

We use **Cargo** with custom aliases to wrap platform-specific shell scripts.

| Platform | Command | Output Artifact | Script Location |
| :--- | :--- | :--- | :--- |
| **Linux** | `cargo bundle-linux` | `onyx.flatpak` | `scripts/bundle_flatpak.sh` |
| **macOS** | `cargo bundle-mac` | `dist/Onyx.dmg` | `scripts/bundle_macos.sh` |

*Note: The commands run wrapper binaries (`src/bin/bundle_*.rs`) which execute the shell scripts.*

---

## 🐧 Linux Pipeline (GitHub Actions Recommended)

We use **Flatpak** for Linux distribution.

### Requirements
*   **Runner**: `ubuntu-latest`
*   **Packages**: `flatpak`, `flatpak-builder`
*   **Runtime**: `org.freedesktop.Platform//25.08`, `org.freedesktop.Sdk//25.08`, `org.freedesktop.Sdk.Extension.rust-stable//25.08`

### Proposed Workflow Steps
1.  **Checkout Code**
2.  **Install Flatpak**:
    ```yaml
    - name: Install Flatpak
      run: |
        sudo apt-get update
        sudo apt-get install -y flatpak flatpak-builder
        flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
        flatpak install --user -y flathub org.freedesktop.Sdk//25.08 org.freedesktop.Platform//25.08 org.freedesktop.Sdk.Extension.rust-stable//25.08
    ```
3.  **Build Bundle**:
    ```yaml
    - name: Build Flatpak
      run: cargo bundle-linux
    ```
4.  **Upload Artifact**: Upload the resulting `onyx.flatpak` file.

---

## 🍎 macOS Pipeline

We use a custom shell script to bundle the `.app` and create a `.dmg`.

### Requirements
*   **Runner**: `macos-latest` (Apple Silicon `macos-14` preferred if available, else standard)
*   **Packages**: `cargo-bundle`, `create-dmg` (install via Homebrew)
*   **Certificates**: The current script uses ad-hoc signing (`-`) for local builds. For production/notarization, you will need to inject a valid Developer ID Application certificate.

### Proposed Workflow Steps
1.  **Checkout Code**
2.  **Install Dependencies**:
    ```yaml
    - name: Install Utils
      run: |
        brew install create-dmg
        cargo install cargo-bundle
    ```
3.  **Build DMG**:
    ```yaml
    - name: Build DMG
      run: cargo bundle-mac
    ```
4.  **Upload Artifact**: Upload `target/release/bundle/osx/Onyx.dmg`.

---

## 🔐 Secrets & Environment Variables

If we move to signed releases (recommended), you will need to configure these secrets in the repo:

*   `MACOS_CERTIFICATE`: Base64 encoded p12 certificate.
*   `MACOS_CERTIFICATE_PWD`: Password for the p12.
*   `MACOS_IDENTITY_ID`: The Common Name of the signing identity (e.g., "Developer ID Application: Onyx Team").

*Note: The current `scripts/bundle_macos.sh` ignores these and forces ad-hoc signing. You will need to uncomment the signing logic in that script when ready.*

---

## 🧪 Testing

Ensure every PR runs the unit tests before building artifacts.

```yaml
- name: Run Tests
  run: cargo test
```

---

## 📦 Release Strategy

1.  **Trigger**: On push to `v*` tags (e.g., `v1.0.0`).
2.  **Build**: Run both Linux and macOS pipelines in parallel.
3.  **Release**: Create a GitHub Release and attach `onyx.flatpak` and `Onyx.dmg`.

Let us know if you have any questions!
