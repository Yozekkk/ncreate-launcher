# Stage 2 verification

This document records observed behavior of the 0.2.0 source tree, not a published release. The public 0.1.0 release and its download links remain unchanged. An actual owner-authenticated Microsoft or Ely.by login has not been performed. The launcher uses a separate NCreate data directory; tests used an isolated temporary directory and did not touch the Modrinth App directory.

## Authentication

| Area | Evidence | Result |
|---|---|---|
| Microsoft flow | Official Microsoft sign-in page opened in the isolated WebView. OAuth domain/TLS, PKCE/state, callback origin/state/expiry/cancellation, invalid responses, XSTS and entitlement/profile failures, refresh/expiry and native vault lifecycle have deterministic tests. | Technical flow verified; real owner login remains blocked. |
| Microsoft mock pipeline | The production protocol implementation was driven through synthetic OAuth callback, token, SISU, XSTS, Minecraft token, entitlements and profile, then account persistence, restart, refresh and logout. | Pass as a mock only. |
| Microsoft real account | No account credentials or browser data were collected. | Blocked: interactive owner login required. |
| Ely.by skins | Real public nickname and absent nickname lookups returned a valid PNG or fallback; malformed/oversized image, redirect, timeout, offline, cache and invalidation paths have tests. The desktop rendered an Ely.by head for `ErickSkrauch` and a fallback for an absent nickname. | Pass for public skins. |
| Ely.by auth protocol | Official Ely.by endpoints and invalid credentials/token paths were exercised. The Accounts UI cleared a synthetic invalid password after failure. The native vault stores issued credentials, never the password; OAuth registration is external configuration. | Protocol/failure paths verified; successful live login unverified. |
| Ely.by real account | No test account was provided. | Blocked: account owner credentials required. |

## Library, content and runtime

The Linux desktop app opened with the NCreate title/icon and rendered Home, Library, Content, Accounts and Settings. Three official edition cards remained disabled because no production manifest URLs exist. Offline accounts and stable/beta settings persisted after restart. Creating, renaming, duplicating, deleting, selecting a native icon, exporting and re-importing a custom `.mrpack` were exercised in the desktop UI. The native picker restricted import/export/icon access to the selected path.

Live public Modrinth API checks searched a known mod, fetched project/versions, resolved a version compatible with Fabric 1.21.1 and identified an imported file using the documented hash batch endpoint. Sodium was installed with a verified hash, disabled/enabled, downgraded, updated through the UI, and rolled back. A public 1.21.1 modpack was imported as a new instance with 19 installed mods. Compatible mod updates and official manifest v1→v2/rollback also have controlled fixture tests; published NCreate editions are not yet available. Modpack version replacement remains unimplemented; individual imported mods can be identified and managed.

Vanilla 1.21.1 and Fabric 1.21.1 were installed from public Minecraft/loader metadata and launched real Minecraft GUI windows using an offline test profile. The Minecraft install pipeline verifies client, library and asset hashes. A Quilt loader install and Play command were observed, but its game window was not independently captured. Forge 1.21.1 completed a separate live installation test. Its client patch has no upstream checksum sidecar, so the patch is downloaded only from the exact trusted metadata path with a 16 MiB bound; the Forge processor's generated outputs are checked against declared SHA-1 values. Upstream wraps these expected hashes in single quotes; normalization accepts exactly one quote pair and rejects invalid hashes. The same ignored test launched the Forge process with an offline test identity and observed it running after 20 seconds on a virtual display; a Forge game-window screenshot was not captured. NeoForge version availability is constrained by the current upstream loader manifest; no version is fabricated. Java discovery verifies the required major version and accepts a custom executable; managed JRE download is not implemented.

## Source checks and packages

The final local sequence passed locked dependency installation, frontend typecheck/lint/Prettier/13 tests/build, Rust fmt/check/70 normal tests/clippy for the three owned crates, and the Stage 2 boundary audit. Separate ignored tests checked the native vault, live Ely.by lookups and the full Forge install/launch. The Linux Tauri production build produced a 155 MiB AppImage and 7.2 MiB amd64 DEB. The AppImage opened on a virtual X11 desktop; its Home screenshot showed the production NCreate UI, five sidebar destinations and three unavailable official editions. In an isolated XDG directory it created only `data/ncreate-launcher` and `data/com.ncreate.launcher`; after 86 seconds, the AppImage process showed 0.3% average CPU. Packaging required running `linuxdeploy` outside the restricted command sandbox; two sandboxed attempts failed inside that tool. No credentials are required in CI. Windows NSIS is built by GitHub Actions but Windows GUI/install behavior needs a physical Windows test.

Local artifact checksums from 2026-09-28:

```text
7b686f69833dcfbd194a4e3aa0b2d766c96975778a9fcbd6fc2d55221ba342a3  target/release/bundle/appimage/NCreate-Launcher-0.2.0.AppImage
9fc123319a99b3491356814f79952d1fa25e2096ed4bd1fad4f21b011650e31a  target/release/bundle/deb/NCreate-Launcher-0.2.0-amd64.deb
```

Run the bounded live Forge smoke test manually when Minecraft download bandwidth is available:

```bash
cargo test --locked -p ncreate-launcher-core --test live_forge -- --ignored --nocapture
```

The test uses a temporary NCreate engine/database and does not run in normal CI. A normal `cargo test` only compiles it.

## Screenshots

Real desktop captures are in [`docs/screenshots/stage2`](screenshots/stage2): [Home](screenshots/stage2/home.png), [Library](screenshots/stage2/library.png), [Create instance](screenshots/stage2/create-instance.png), [Mods](screenshots/stage2/content-mods.png), [Modpack browsing](screenshots/stage2/modpacks.png), [Instance mods](screenshots/stage2/instance-mods.png), [Update dialog](screenshots/stage2/update-dialog.png), [Accounts](screenshots/stage2/accounts.png), [Settings](screenshots/stage2/settings.png), [Vanilla game](screenshots/stage2/minecraft-vanilla.png), and [Fabric game](screenshots/stage2/minecraft-fabric.png). Both standard and smaller desktop window sizes were reviewed for clipping, overflow, loading/error/empty/disabled states and focus/hover controls. A developer bridge was used only in debug verification and is excluded from release builds.

## Boundaries and remaining work

Network destinations and excluded Modrinth production surfaces are listed in [NETWORK.md](NETWORK.md). The public Modrinth content API and loader metadata/Maven hosts are the only Modrinth domains required by Stage 2. No Modrinth account, ads, Hosting, Friends, analytics, Sentry, PostHog or original updater are used. The official edition provider map starts empty; setting a real NCreate HTTPS manifest endpoint is a separate content publication step. The upstream fallback for a loader version removed from remote metadata has not yet been adapted, so such an already installed version can require recovery. No Stage 2 release has been published.
