# Soffit Commit

A private, local-first workspace for working with commits, documents, spreadsheets, and offline AI assistance.

## Downloads

Download the current version from the [latest GitHub release](https://github.com/gmtechs/Soffit-Commit/releases/latest). Choose the asset matching your operating system and processor.

| Platform | Release asset | Best for |
| --- | --- | --- |
| Windows x64 | `.exe` setup installer | Most Windows PCs — recommended. |
| Windows x64 | `.msi` installer | Managed or enterprise PCs. |
| Linux x64 | `.AppImage` | Most Linux distributions; make it executable, then open it. |
| Debian/Ubuntu x64 | `.deb` package | Debian, Ubuntu, Linux Mint, and related systems. |
| macOS Intel | `x64.dmg` | Intel-based Macs. |
| macOS Apple silicon | `aarch64.dmg` | M1, M2, M3, M4, and later Macs. |

> Releases are assembled as drafts. Open the draft on GitHub, confirm all six assets are present, then publish it. Windows and macOS installers are not code-signed in this repository yet, so the operating system may show a trust warning. Add signing credentials before distributing to end users.

## Release builds

Pushing a version tag such as `app-v0.1.1` starts native GitHub Actions builds for Windows x64, Linux x64, Intel macOS, and Apple-silicon macOS. The workflow attaches the installers above to one draft GitHub release. This avoids attempting to cross-compile macOS installers on Linux.

## Development

```bash
npm install
cargo tauri dev
```

## Build

```bash
cargo tauri build
```
