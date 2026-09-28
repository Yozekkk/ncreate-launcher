<p align="center">
  <a href="README.md">Русский</a> •
  <strong>English</strong>
</p>

<div align="center">
  <img src="apps/app-frontend/public/brand/logo.webp" alt="NCreate logo" width="112" height="112">
  <h1>NCreate Launcher</h1>
  <p>A desktop Minecraft launcher for the NCreate project on Windows and Linux.</p>
  <p>
    <img src="https://img.shields.io/badge/target-v0.5.0-f59e0b?style=flat-square" alt="Development target: v0.5.0">
    <a href="https://github.com/Yozekkk/ncreate-launcher/releases/latest"><img src="https://img.shields.io/github/v/release/Yozekkk/ncreate-launcher?style=flat-square&color=f59e0b&label=release" alt="Latest published release"></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-555?style=flat-square" alt="Windows 10 and 11 x64">
    <img src="https://img.shields.io/badge/Linux-x64-555?style=flat-square&logo=linux&logoColor=white" alt="Linux x64">
    <a href="https://github.com/Yozekkk/ncreate-launcher/actions/workflows/check.yml"><img src="https://img.shields.io/github/actions/workflow/status/Yozekkk/ncreate-launcher/check.yml?branch=main&style=flat-square&label=CI" alt="CI checks"></a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Tauri-v2-555?style=flat-square&logo=tauri&logoColor=f59e0b" alt="Tauri v2">
    <img src="https://img.shields.io/badge/Rust-555?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Vue-3-555?style=flat-square&logo=vuedotjs&logoColor=white" alt="Vue 3">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPLv3-555?style=flat-square" alt="GPLv3 license"></a>
  </p>
</div>

## Download

> [!IMPORTANT]
> **v0.5.0 is a development target, not a published release.** The source code in `main` is currently version `0.2.0`. The latest public release is **v0.1.0 Beta**. v0.5.0 installers do not exist yet; the links below point only to real v0.1.0 files. That release does not include the Library, mod catalog, or Minecraft launch features described below for the current source tree.

| Platform                                                           | Available v0.1.0 Beta file         | Download                                                                                                                   |
| :----------------------------------------------------------------- | :--------------------------------- | :------------------------------------------------------------------------------------------------------------------------- |
| **Windows 10/11 x64**                                              | `NCreate-Launcher-Setup-0.1.0.exe` | [Windows installer](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.1.0/NCreate-Launcher-Setup-0.1.0.exe) |
| **Linux x64** — EndeavourOS, Arch, Fedora, and other distributions | `NCreate-Launcher-0.1.0.AppImage`  | [AppImage](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.1.0/NCreate-Launcher-0.1.0.AppImage)           |
| **Debian, Ubuntu, Linux Mint**, and compatible systems             | `NCreate-Launcher-0.1.0-amd64.deb` | [DEB](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.1.0/NCreate-Launcher-0.1.0-amd64.deb)               |

[All releases](https://github.com/Yozekkk/ncreate-launcher/releases) · [Latest release](https://github.com/Yozekkk/ncreate-launcher/releases/latest) · [Checksums for published files](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.1.0/SHA256SUMS.txt)

The expected v0.5.0 filenames are `NCreate-Launcher-Setup-0.5.0.exe`, `NCreate-Launcher-0.5.0.AppImage`, and `NCreate-Launcher-0.5.0-amd64.deb`. Direct links will be added only after the builds and release have been verified.

## What's new in v0.5.0

This is a **presentation plan for v0.5.0** based on features already verified in the current `main` source tree (`0.2.0`). It does not describe the published v0.1.0 installers.

- **Library:** separate Minecraft instances, creation, renaming, duplication, and `.mrpack` import and export.
- **Game runtime:** Vanilla and Fabric installation and launch were verified on Linux. Forge installation and its running game process were also checked; Quilt and NeoForge have verification limits noted below.
- **Mods and modpacks:** public Modrinth catalog, search, compatible installation, and mod enable, disable, removal, update, and rollback.
- **Accounts:** local profiles, Microsoft and Ely.by sign-in protocols, and public Ely.by skins.
- **NCreate groundwork:** separate manifests, file verification, download management, and recovery after failed updates. The official Minimal, Standard, and Ultra editions are still unavailable.

The source tree passed [Linux and Windows CI](https://github.com/Yozekkk/ncreate-launcher/actions/runs/36422468689). That result does not verify future v0.5.0 binaries.

## Preview

These are real Linux desktop captures from the current `main` branch.

<p align="center">
  <img src="docs/screenshots/stage2/home.png" alt="NCreate Launcher home with Minimal, Standard, and Ultra" width="900">
</p>

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/stage2/library.png" alt="Minecraft instance library"></td>
    <td width="50%"><img src="docs/screenshots/stage2/content-mods.png" alt="Mod catalog"></td>
  </tr>
  <tr><td align="center">Library</td><td align="center">Mods and modpacks</td></tr>
  <tr>
    <td><img src="docs/screenshots/stage2/accounts.png" alt="Accounts page"></td>
    <td><img src="docs/screenshots/stage2/settings.png" alt="Launcher settings"></td>
  </tr>
  <tr><td align="center">Accounts</td><td align="center">Settings</td></tr>
</table>

## Features

The current source tree includes separate Minecraft instances, mod and modpack discovery, local accounts, and settings. The Library and catalog use the NCreate interface; advertising and a Modrinth account are not required. These features are not present in the published v0.1.0 installers yet.

## Library

The current source lets you create separate Vanilla, Fabric, Forge, and Quilt instances. You can rename, duplicate, delete, import, and export them as `.mrpack` files. Each instance has its own mods, memory setting, and Java path. NeoForge selection is also present, but an end-to-end launch has not been verified; compatible versions depend on available loader metadata.

## Mods and modpacks

The launcher uses the public Modrinth catalog to find mods and modpacks. You can install a compatible version into an instance, manage installed mods, and import a `.mrpack` as a new instance. Updating and rolling back an individual mod have been verified. Automatic **whole-modpack version updates** are not implemented yet.

Modrinth is a content source here. **NCreate Launcher is not the official Modrinth App** and does not require a Modrinth account to browse public content.

## Accounts

| Type          | Support in the current source tree                                                                                                                                                        |
| :------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Offline**   | Local nickname and stable deterministic UUID. Creation, switching, and persistence across restart were verified.                                                                          |
| **Microsoft** | The official OAuth/Minecraft flow is implemented. Technical checks and a complete synthetic pipeline passed, but a successful sign-in with an owner's real account has not been verified. |
| **Ely.by**    | The sign-in protocol and skin support are implemented. Public skin lookup and fallback were checked live; successful sign-in with a real Ely.by account remains unverified.               |

Offline and Ely.by profiles do not grant access to servers that require a licensed Microsoft account. See the [Stage 2 verification report](docs/STAGE2-VERIFICATION.md) for the evidence boundaries.

## Official NCreate editions

Minimal, Standard, and Ultra remain distinct NCreate editions. Their cards appear on Home, but installation buttons are disabled: **the official NCreate packs are still in preparation**. Manifest and update handling passed fixture tests; production manifests have not been published.

Automatic launcher updates are currently disabled until signed NCreate releases are available.

## Installation

Download an **existing v0.1.0 file** from the table above. The v0.5.0 filenames and commands below apply after that version is published; until then, replace `0.5.0` with `0.1.0`.

### Windows 10/11 x64

Download the `.exe`, run the installer, and open NCreate Launcher. The beta installer does not yet have a commercial code-signing certificate, so Windows SmartScreen may warn you. Check the download source and checksum; keep Windows security protection enabled. Windows CI builds NSIS, but installation and GUI behavior on a physical Windows machine have not been verified.

### Linux AppImage

AppImage is the recommended portable option for EndeavourOS, Arch Linux, Fedora, and other modern distributions. Once the v0.5.0 file exists, run:

```bash
chmod +x NCreate-Launcher-0.5.0.AppImage
./NCreate-Launcher-0.5.0.AppImage
```

If your system lacks FUSE 2, use `APPIMAGE_EXTRACT_AND_RUN=1 ./NCreate-Launcher-0.5.0.AppImage`.

### Debian, Ubuntu, and Linux Mint

The DEB package is for Debian/Ubuntu-based systems. Once the v0.5.0 file exists, install it with:

```bash
sudo apt install ./NCreate-Launcher-0.5.0-amd64.deb
```

Choose AppImage instead of DEB on EndeavourOS and Arch.

### Verify a download

Download `SHA256SUMS.txt` from the same release and compare your file's hash with its entry. For the available v0.1.0 AppImage on Linux:

```bash
sha256sum NCreate-Launcher-0.1.0.AppImage
```

For the available v0.1.0 installer on Windows:

```powershell
Get-FileHash .\NCreate-Launcher-Setup-0.1.0.exe -Algorithm SHA256
```

## First launch

These steps apply to the **current source tree** and a future release containing its features. The public v0.1.0 build has no Library or game launch.

1. Open NCreate Launcher and add a profile in **Accounts**.
2. Create a Minecraft instance in **Library**, or import a `.mrpack`.
3. Find compatible mods in **Mods and modpacks** if you want them.
4. Install Minecraft for the selected instance, then select **Play**.

## System requirements

- Windows 10/11 x64 or Linux x64; Windows GUI behavior still needs a direct test.
- Free space for Minecraft and your selected mods; the amount depends on the game version and content.
- Internet access for the first game, loader, and content downloads.
- A Java version compatible with your Minecraft version. The launcher checks installed Java and accepts a custom executable path; managed Java downloads are not implemented yet.

## Privacy

The app has no advertising or telemetry sent to Modrinth, PostHog, or Sentry. Account metadata and settings stay in a separate NCreate data directory; existing Modrinth App data is not imported. Microsoft and Ely.by tokens are stored in the operating system's credential store and are not sent to the frontend. Passwords are not saved: an Ely.by password, when required for sign-in, is used only during that request. See the [network audit](docs/NETWORK.md).

## Development

You need Node.js 22.12+, pnpm 10.30.3, stable Rust, and the [Tauri Linux prerequisites](docs/DEVELOPMENT.md).

```bash
git clone https://github.com/Yozekkk/ncreate-launcher.git
cd ncreate-launcher
pnpm install --frozen-lockfile
pnpm app:dev
```

Build Linux AppImage and DEB packages with:

```bash
pnpm app:build --bundles appimage,deb
```

On Windows, use `pnpm app:build --bundles nsis`. The source is still version `0.2.0`; building it **does not create a v0.5.0 installer**. See the [development guide](docs/DEVELOPMENT.md) for details.

## Documentation

- [Development and packaging](docs/DEVELOPMENT.md)
- [Network and privacy](docs/NETWORK.md)
- [Stage 2 architecture](docs/STAGE2-ARCHITECTURE.md)
- [Stage 2 verification](docs/STAGE2-VERIFICATION.md)
- [Requirements and limitations](docs/REQUIREMENTS.md)
- [Release process](docs/RELEASE.md)
- [Upstream source audit](docs/UPSTREAM-AUDIT.md)
- [Prepared v0.5.0 notes](docs/releases/v0.5.0.en.md)

## License and credits

NCreate Launcher uses modified portions of the open-source [Modrinth App](https://github.com/modrinth/code) under [GPLv3](LICENSE). Original notices and attribution remain in [NOTICE.md](NOTICE.md), [COPYING.md](COPYING.md), and the [upstream audit](docs/UPSTREAM-AUDIT.md). NCreate owns its branding. This project is not affiliated with Mojang or Microsoft.
