# Soffit Commit

A private, local-first workspace for working with commits, documents, spreadsheets, and offline AI assistance.

## Downloads

Download the current version from the [latest GitHub release](https://github.com/gmtechs/Soffit-Commit/releases/latest). Choose the asset matching your operating system and processor.

| Platform | Release asset | Download |
| --- | --- | --- |
| Windows x64 | `Soffit.Commit_0.1.4_x64-setup.exe` setup installer — recommended for most PCs | [Download for Windows](https://github.com/gmtechs/Soffit-Commit/releases/download/app-v0.1.4/Soffit.Commit_0.1.4_x64-setup.exe) |
| Windows x64 | `Soffit.Commit_0.1.4_x64_en-US.msi` installer for managed PCs | [Download the MSI installer](https://github.com/gmtechs/Soffit-Commit/releases/download/app-v0.1.4/Soffit.Commit_0.1.4_x64_en-US.msi) |
| Linux x64 | `Soffit.Commit_0.1.4_amd64.AppImage` — make it executable, then open it | [Download for Linux](https://github.com/gmtechs/Soffit-Commit/releases/download/app-v0.1.4/Soffit.Commit_0.1.4_amd64.AppImage) |
| Debian/Ubuntu x64 | `Soffit.Commit_0.1.4_amd64.deb` package | [Download the Debian package](https://github.com/gmtechs/Soffit-Commit/releases/download/app-v0.1.4/Soffit.Commit_0.1.4_amd64.deb) |
| macOS Apple silicon | `Soffit.Commit_0.1.4_aarch64.dmg` for M-series Macs | [Download for Apple silicon Macs](https://github.com/gmtechs/Soffit-Commit/releases/download/app-v0.1.4/Soffit.Commit_0.1.4_aarch64.dmg) |

> The Intel macOS (`.dmg`) build was still queued when this README was updated, so it is not listed above. It will appear on the [app-v0.1.4 release page](https://github.com/gmtechs/Soffit-Commit/releases/tag/app-v0.1.4) once its GitHub Actions job finishes. Windows and macOS installers are not code-signed in this repository yet, so the operating system may show a trust warning. Add signing credentials before distributing to end users.

## Release builds

Pushing a version tag such as `app-v0.1.4` starts native GitHub Actions builds for Windows x64, Linux x64, Intel macOS, and Apple-silicon macOS. The workflow attaches the installers above to one draft GitHub release. This avoids attempting to cross-compile macOS installers on Linux.

## Development

```bash
npm install
cargo tauri dev
```

## Testing with a second local device

Use the normal command above for your first device. To run a second, independent
development device on the same computer, start this launcher in another terminal:

```bash
./scripts/run-second-instance.sh
```

It runs the frontend on `127.0.0.1:1422` (the primary instance uses `1420`) and stores
the second device's database, settings, and Iroh identity in
`.tauri-test-instance/`. The two windows therefore behave like two separate PCs
for account, pairing, and peer-to-peer testing.

To keep the second device running after closing the terminal or ending an agent
session, run it in a detached tmux session:

```bash
tmux new-session -d -s soffit-second './scripts/run-second-instance.sh 2>&1 | tee logs/second-instance.log'
```

Watch its startup output with `tail -f logs/second-instance.log` or attach with
`tmux attach -t soffit-second`. Detach from tmux with `Ctrl-b`, then `d`; stop it
with `tmux kill-session -t soffit-second`. To reset the simulated
device completely, stop it and delete only `.tauri-test-instance/`; this does not
affect the primary instance.

## Build

```bash
cargo tauri build
```
