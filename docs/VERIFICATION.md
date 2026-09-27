# NCreate Launcher stage-one verification

Verified on EndeavourOS/Linux x64 using a real Tauri/WebKitGTK window, including
production AppImage launches on an X11 virtual display. This is a desktop build,
not a browser-only prototype. Screenshots were opened and reviewed; selected
screens are in [screenshots](screenshots). The source site and upstream checkout
were not modified.

## Source and product boundaries

Audited Modrinth main `0e817be41cebdb0e140866e87e727c2101fe0e6b` supplies the
Microsoft OAuth/Xbox/SISU/XSTS/Minecraft pipeline and Tauri/Vue desktop foundation.
The extraction retains only apps/app, apps/app-frontend and packages/app-lib.
Full upstream State starts unrelated product clients, so the auth pipeline is
isolated and local accounts/settings use a separate SQLite store. Original GPL
notices/copyright and inactive auth provenance are preserved.

Marketplace, arbitrary instance/import/play, hosting, ads, friends, news,
Modrinth account login, analytics, Sentry and original updater are absent from
the executable product. [NETWORK.md](NETWORK.md) records retained functional
Microsoft/Xbox/Mojang/Ely.by destinations. No Modrinth production endpoint remains
in active launcher code; the upstream comparison file is not compiled.

## Implementation map

| Concern | Files and behavior |
| --- | --- |
| Branding | apps/app/tauri.conf.json, apps/app/icons, frontend/public/brand, frontend/src/styles.css; official site logo and orange/amber palette |
| Editions | frontend/src/models.ts contains minimal/standard/ultra with null manifests; edition-installer.ts exposes configureEditionInstaller(handler), requiring both handler and manifest; no actual installer/Play backend |
| Offline profiles | apps/app/src/storage.rs; Java-compatible UUID v3 from MD5 of OfflinePlayer:nickname, metadata in SQLite, transactional single-active selection and rename semantics |
| Microsoft | packages/app-lib/src/minecraft_auth.rs, vault.rs, lib.rs and apps/app/src/main.rs; upstream browser OAuth, exact callback/state validation, native credential-store tokens only |
| Skins | apps/app/src/skins.rs; public Ely.by nickname API, HTTPS allowlisted redirects, bounded PNG decoding/cache; Mojang/Ely.by/local provider and bundled Steve fallback |
| Settings | native persisted Russian/theme/motion/blur/future game settings; narrowly scoped autostart plugin; updater disabled |
| Data | Linux ~/.local/share/ncreate-launcher/launcher.db and skins; Tauri window state under ~/.config/com.ncreate.launcher; Windows local AppData NCreate directory and OS credential store |
| Packaging | scripts/build.mjs and .github/workflows; normalized NSIS/AppImage/deb names, v-tag release with GPL source archive and SHA256SUMS |

## Actual checks

Locked pnpm install, frontend typecheck/ESLint/Prettier/build and six adapter tests
pass. Rust formatting, cargo check all targets, clippy with warnings denied and
15 Rust tests pass. [Windows/Linux CI run](https://github.com/Yozekkk/ncreate-launcher/actions/runs/36256748738)
succeeded and produced real installers; their artifacts were downloaded.
Current main CI is available from [Actions](https://github.com/Yozekkk/ncreate-launcher/actions).

Desktop testing covered Home, Accounts empty/ready, offline validation, profile
cabinet, Microsoft progress/official sign-in, Settings, 860×620 resizing/scroll,
keyboard focus/Tab/Escape, two offline profiles, activation, rename, removal,
persistence and native restart. A real Ely.by skin and a missing-skin Steve
fallback were rendered. Network-banner testing used a simulated navigator offline
event, not a physical network disconnect. Autostart enable/disable state worked;
a full logout/login cycle was not tested. Release bridge probes found no bridge.
Local test profiles were removed and default theme/motion restored.

## Commands and local artifacts

From the checked out repository:

```sh
cd /home/tima/ncreate-launcher
pnpm install --frozen-lockfile
pnpm app:dev
```

Production build:

```sh
cd /home/tima/ncreate-launcher
pnpm app:build --bundles appimage,deb
```

Local artifacts:

- target/release/bundle/appimage/NCreate-Launcher-0.1.0.AppImage (154 MiB)
- target/release/bundle/deb/NCreate-Launcher-0.1.0-amd64.deb (5.8 MiB)
- Windows CI: NCreate-Launcher-Setup-0.1.0.exe

Run the AppImage directly, or use APPIMAGE_EXTRACT_AND_RUN=1 if FUSE is absent.
Local Arch builds use the host glibc baseline; use Ubuntu CI artifacts for an older
baseline. Rust release strips the executable; NO_STRIP keeps modern dependency
ELF sections compatible with linuxdeploy. patchelf is a required native build tool.

## Honest remaining limits

**PARTIAL / BLOCKED:** Microsoft reaches the official account page and callback/
vault tests pass, but saving/refreshing a real authenticated Microsoft profile
requires the account owner's interactive authentication. No credentials were
entered or simulated.

**PARTIAL:** Windows NSIS build passes; actual installation and GUI behavior on
Windows are not verified on this Linux host. Install/run/uninstall on Windows
x64 is the next platform validation step. Tag release publication is configured,
but no release tag was published during this task.

**NOT IMPLEMENTED by requested scope:** real Minecraft edition install/launch,
full Ely.by launcher authentication and English translation. Automatic updates
are safely disabled; future activation/signing needs genuine private signing keys
and Windows Authenticode credentials. See RELEASE.md for exact secrets and steps.

Repository: https://github.com/Yozekkk/ncreate-launcher. Use git rev-parse HEAD for
the final main revision; the final delivery message records the pushed SHA.
