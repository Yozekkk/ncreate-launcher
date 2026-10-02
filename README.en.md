<p align="center">
  <a href="README.md">Русский</a> •
  <strong>English</strong>
</p>

<div align="center">
  <img src="apps/app-frontend/public/brand/logo.webp" alt="NCreate logo" width="112" height="112">
  <h1>NCreate Launcher v0.6.0 Beta</h1>
  <p>A desktop Minecraft launcher for the NCreate project on Windows and Linux.</p>
  <p>
    <img src="https://img.shields.io/badge/version-v0.6.0%20Beta-f59e0b?style=flat-square" alt="Version v0.6.0 Beta">
    <a href="https://github.com/Yozekkk/ncreate-launcher/releases/tag/v0.6.0"><img src="https://img.shields.io/badge/release-v0.6.0%20Beta-f59e0b?style=flat-square" alt="v0.6.0 Beta release"></a>
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
> **v0.6.0 Beta** is the first release with the official **NCreate Server 1.0.2** pack. Install it from Home: the launcher downloads Minecraft, NeoForge, mods, and configurations. Users of v0.5.0 need to install v0.6.0 manually once; later signed updates can be checked in the launcher.

| Platform                                                                    | v0.6.0 Beta file                   | Download                                                                                                                          |
| :-------------------------------------------------------------------------- | :--------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------- |
| **Windows 10/11 x64**                                                       | `NCreate-Launcher-Setup-0.6.0.exe` | **[Download for Windows](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/NCreate-Launcher-Setup-0.6.0.exe)** |
| **Linux x64** — EndeavourOS, Arch, Manjaro, Fedora, and other distributions | `NCreate-Launcher-0.6.0.AppImage`  | **[Download AppImage](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/NCreate-Launcher-0.6.0.AppImage)**     |
| **Debian, Ubuntu, Linux Mint**, and compatible systems                      | `NCreate-Launcher-0.6.0-amd64.deb` | **[Download DEB](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/NCreate-Launcher-0.6.0-amd64.deb)**         |

[v0.6.0 release notes](https://github.com/Yozekkk/ncreate-launcher/releases/tag/v0.6.0) · [All releases](https://github.com/Yozekkk/ncreate-launcher/releases) · [SHA-256 checksums](https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/SHA256SUMS.txt)

## What's new in v0.6.0

This release makes the official server pack available directly in the launcher. The custom-instance Library and mod catalog remain available.

- **Library:** separate Minecraft instances, creation, renaming, duplication, and `.mrpack` import and export.
- **Game runtime:** Vanilla, Fabric, and the Forge game process were verified on Linux. The official pack's NeoForge 21.1.250 installation and game-process launch were also checked; connecting to the server was not tested separately. Quilt was installed, but its GUI launch was not checked separately.
- **Mods and modpacks:** public Modrinth catalog, search, compatible installation, and mod enable, disable, removal, update, and rollback.
- **Accounts:** local profiles, Microsoft and Ely.by sign-in protocols, and public Ely.by skins.
- **NCreate Server:** one official pack with a published manifest, SHA-256 verification, user-file preservation, and recovery after interrupted updates. Downloading all 718 managed files and launching the pack were verified on Linux.

Feature checks are detailed in the [Stage 2 verification report](docs/STAGE2-VERIFICATION.md). A successful sign-in with a real Microsoft or Ely.by account still requires an owner test.

## Preview

These are real Linux desktop captures. Home shows the official pack; the other images show the retained Library, catalog, accounts, and settings pages.

<p align="center">
  <img src="docs/screenshots/official-home.png" alt="NCreate Launcher home with the official NCreate Server pack" width="900">
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

Version 0.6.0 includes the official NCreate Server installation, separate Minecraft instances, mod and modpack discovery, accounts, and settings. The Library and catalog use the NCreate interface; advertising and a Modrinth account are not required.

## Library

You can create separate Vanilla, Fabric, Forge, Quilt, and NeoForge instances. You can rename, duplicate, delete, import, and export them as `.mrpack` files. Each instance has its own mods, memory setting, and Java path. NeoForge 21.1.250 installation and launch were verified with the official pack; other versions depend on available loader metadata.

## Mods and modpacks

The launcher uses the public Modrinth catalog to find mods and modpacks. You can install a compatible version into an instance, manage installed mods, and import a `.mrpack` as a new instance. Updating and rolling back an individual mod have been verified. Automatic updates of **arbitrary third-party modpacks** are not implemented yet; the official NCreate Server pack updates through its own manifest.

Modrinth is a content source here. **NCreate Launcher is not the official Modrinth App** and does not require a Modrinth account to browse public content.

## Accounts

| Type          | Support in v0.6.0 Beta                                                                                                                                                                    |
| :------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Offline**   | Local nickname and stable deterministic UUID. Creation, switching, and persistence across restart were verified.                                                                          |
| **Microsoft** | The official OAuth/Minecraft flow is implemented. Technical checks and a complete synthetic pipeline passed, but a successful sign-in with an owner's real account has not been verified. |
| **Ely.by**    | The sign-in protocol and skin support are implemented. Public skin lookup and fallback were checked live; successful sign-in with a real Ely.by account remains unverified.               |

Offline and Ely.by profiles do not grant access to servers that require a licensed Microsoft account. See the [Stage 2 verification report](docs/STAGE2-VERIFICATION.md) for the evidence boundaries.

## Official NCreate Server pack

Home offers one [official NCreate Server pack](https://github.com/Yozekkk/ncreate-pack). Its Stable manifest has been published: Minecraft 1.21.1, NeoForge 21.1.250, 147 mods from verified upstream sources, configuration files, and the server address from the source pack. Updates download changed managed files, verify SHA-256, and preserve user mods, worlds, and settings. The Library marks the installed pack as “Official NCreate”; custom instances remain available.

The pack needs Java 21 for installation and first launch. The launcher can use an installed Java runtime or a manually selected path; managed Java downloads are not implemented yet. Users of v0.5.0 need to download and install v0.6.0 manually once because the older binary does not include the self-updater.

## Installation

Choose the v0.6.0 package for your system in the table above and install it as follows.

### Windows 10/11 x64

Download the `.exe`, run the installer, and open NCreate Launcher. The beta installer does not yet have a commercial code-signing certificate, so Windows SmartScreen may warn you. Check the download source and checksum; keep Windows security protection enabled. Windows CI builds NSIS, but installation and GUI behavior on a physical Windows machine have not been verified.

### Linux AppImage

AppImage is the recommended portable option for EndeavourOS, Arch Linux, Manjaro, Fedora, and other modern distributions. After downloading the file, run:

```bash
chmod +x NCreate-Launcher-0.6.0.AppImage
./NCreate-Launcher-0.6.0.AppImage
```

If the file is in `~/Downloads`, first run `cd ~/Downloads`.

If your system lacks FUSE 2, use `APPIMAGE_EXTRACT_AND_RUN=1 ./NCreate-Launcher-0.6.0.AppImage`.

### Debian, Ubuntu, and Linux Mint

The DEB package is for Debian/Ubuntu-based systems. After downloading the file, install it with:

```bash
cd ~/Downloads
sudo apt install ./NCreate-Launcher-0.6.0-amd64.deb
```

Choose AppImage instead of DEB on EndeavourOS and Arch.

### Verify a download

Download `SHA256SUMS.txt` from the same release and compare your file's hash with its entry. For the v0.6.0 AppImage on Linux:

```bash
sha256sum NCreate-Launcher-0.6.0.AppImage
```

For the v0.6.0 installer on Windows:

```powershell
Get-FileHash .\NCreate-Launcher-Setup-0.6.0.exe -Algorithm SHA256
```

## First launch

After installation:

1. Open NCreate Launcher and add a profile in **Accounts**.
2. Select **Install** for the official NCreate Server pack on **Home**, or create a custom instance in **Library**.
3. Let the launcher install Minecraft, the loader, and pack files.
4. Find compatible mods in **Mods and modpacks** if you want them, then select **Play**.

## System requirements

- Windows 10/11 x64 or Linux x64; Windows GUI behavior still needs a direct test.
- Free space for Minecraft and your selected mods; the amount depends on the game version and content.
- Internet access for the first game, loader, and content downloads.
- A Java version compatible with your Minecraft version; NCreate Server requires Java 21. The launcher checks installed Java and accepts a custom executable path; managed Java downloads are not implemented yet.

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

On Windows, use `pnpm app:build --bundles nsis`. See the [development guide](docs/DEVELOPMENT.md) for details.

## Documentation

- [Development and packaging](docs/DEVELOPMENT.md)
- [Network and privacy](docs/NETWORK.md)
- [Stage 2 architecture](docs/STAGE2-ARCHITECTURE.md)
- [Stage 2 verification](docs/STAGE2-VERIFICATION.md)
- [Requirements and limitations](docs/REQUIREMENTS.md)
- [Release process](docs/RELEASE.md)
- [Launcher and official pack updates](docs/UPDATES.md)
- [Official server pack and verification](docs/OFFICIAL-PACK.md)
- [Upstream source audit](docs/UPSTREAM-AUDIT.md)
- [v0.6.0 release notes](docs/releases/v0.6.0.en.md)

## License and credits

NCreate Launcher uses modified portions of the open-source [Modrinth App](https://github.com/modrinth/code) under [GPLv3](LICENSE). Original notices and attribution remain in [NOTICE.md](NOTICE.md), [COPYING.md](COPYING.md), and the [upstream audit](docs/UPSTREAM-AUDIT.md). NCreate owns its branding. This project is not affiliated with Mojang or Microsoft.
