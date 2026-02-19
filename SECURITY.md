# 🛡️ Security & Compliance Report

This document provides a summary of the security posture, vulnerability assessments, and Bill of Materials (BOM) for the Onyx Downloader project.

---

## 🔐 Vulnerability Assessment

We perform regular security audits of our dependencies using `cargo-audit`.

### Summary Status
![Security Status](https://img.shields.io/badge/Security-Scanned-success?style=for-the-badge&logo=shield)
![Vulnerabilities](https://img.shields.io/badge/Vulnerabilities-0%20Critical-success?style=for-the-badge)
![Warnings](https://img.shields.io/badge/Warnings-2%20Unmaintained-yellow?style=for-the-badge)

### Audit History & Live Results
Security scans are performed on every commit. You can view the live status and detailed history in our [GitHub Actions pipeline](https://github.com/ryanharper/onyx/actions).

#### Known Advisories (Last Reviewed: 2026-02-18)
| Crate | Issue | Mitigation |
| :--- | :--- | :--- |
| `bincode` | Unmaintained | Used only in `iced` debugging tools. |
| `paste` | Unmaintained | Deep dependency; no known vulnerabilities. |

*Note: "Unmaintained" warnings refer to crates that are no longer receiving active updates but do not currently have known exploitable vulnerabilities in our project.*

---

## 📦 Bill of Materials (BOM)

A full Bill of Materials is maintained in CycloneDX format.

*   **Download BOM**: [`BOM.json`](./BOM.json)
*   **Format**: CycloneDX JSON v1.4
*   **Total Components**: 655

### Key Dependencies & Licenses

| Component | Version | License | Description |
| :--- | :--- | :--- | :--- |
| `iced` | 0.14.0 | MIT | GUI Framework |
| `tokio` | 1.49.0 | MIT | Async Runtime |
| `gstreamer` | 0.23.7 | MIT/Apache-2.0 | Media Backbone |
| `reqwest` | 0.12.28 | MIT/Apache-2.0 | HTTP Client |
| `serde` | 1.0.228 | MIT/Apache-2.0 | Serialization |

---

## 🛠️ Security Practices

1.  **Dependency Pinning**: We use `Cargo.lock` to ensure reproducible and vetted builds.
2.  **Automated Scanning**: Our CI/CD pipeline runs `cargo audit` on every pull request.
3.  **Minimal Permissions**: The application is designed to run with minimal system permissions.
4.  **Privacy Focused**: No telemetry or tracking is included in the application.

---

## 📄 Compliance

Onyx Downloader is committed to open-source compliance. All dependency licenses are tracked and verified to be compatible with the MIT license of this project.

For a full list of all 650+ dependencies and their respective licenses, please refer to the [BOM.json](./BOM.json) file.
