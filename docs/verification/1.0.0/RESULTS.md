# NCreate Launcher 1.0.0 Stable — verification

The release candidate was checked on Linux on 2026-10-06. No `v1.0.0` tag or
GitHub Release was published. The original user-owned `обновление осени/`
directory remains untouched and untracked. The [implementation audit](AUDIT.md)
records the OneLauncher mapping and inspection of all seasonal originals.

## Local release gates

All commands below completed with exit code 0. The dependency, frontend and
Rust checks were repeated on `cbe0624`. The local Linux package was built
before its Windows-test-only commits; CI rebuilt the same production sources
from `cbe0624` on both platforms.

| Command | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | PASS |
| `pnpm prepr:frontend:app` | PASS, 14 frontend tests |
| `pnpm build` | PASS |
| `cargo fmt --package ncreate-launcher --package ncreate-app-lib --package ncreate-launcher-core --check` | PASS |
| `cargo check --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets` | PASS |
| `cargo clippy --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets --no-deps -- -D warnings` | PASS |
| `cargo test --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core` | PASS |
| `node --test scripts/release-assets.test.mjs` | PASS, 4 tests |
| `node scripts/check-stage2-boundary.mjs` | PASS |
| `pnpm app:dev` | PASS, real Linux Tauri WebView |
| `pnpm app:build --bundles appimage,deb` | PASS |

The exact command output is in ignored local `verification/stable-1.0.0/logs/`.
The Linux package artifacts are
`target/release/bundle/appimage/NCreate-Launcher-1.0.0.AppImage` and
`target/release/bundle/deb/NCreate-Launcher-1.0.0-amd64.deb`. Both detached
signatures verify with the configured updater public key and embed version
`1.0.0`; a deliberately altered artifact is rejected. The AppImage also started
as a production build with a separate `XDG_DATA_HOME`, initialized a fresh
database, and rendered Home in a real WebView. The production screenshot is in
the ignored verification directory. This test did not touch existing user data.

## Java and Minecraft

The authoritative Mojang version manifest was downloaded and its hashes checked
for all 103 advertised release metadata documents. Their Java requirements
cover majors 8, 16, 17, 21 and 25. The production resolver reads the merged
Minecraft metadata and the explicit NCreate manifest override, then validates
the actual executable and architecture. It does not infer a global Java 21.

| Minecraft / pack | Required Java | Verified Linux path |
| --- | ---: | --- |
| Vanilla 1.16.5 | 8 | metadata → managed runtime → real install → launch → stop |
| Vanilla 1.17 | 16 | metadata → managed runtime → real install → launch → stop |
| Vanilla 1.18 | 17 | metadata → managed runtime → real install → launch → stop |
| Vanilla 1.21.1 | 21 | metadata → managed runtime → real install → launch → stop |
| NCreate Server 1.0.2 / NeoForge 21.1.250 / Minecraft 1.21.1 | 21 | production manifest → install → NeoForge preparation → game launch → stop |

The managed Java 8, 16, 17 and 21 executables were actually downloaded from
Mojang, verified against manifest SHA-1 and size, materialized transactionally,
and executed on Linux. The real five-game pipeline test passed after a Linux
legacy LWJGL issue with non-ASCII native paths was corrected. The official pack
remained alive through the resource-loading window for 60 seconds. Earlier
official-pack runs were killed by Linux OOM and are not counted as passes; the
successful rerun used the manifest-recommended memory after RAM was freed.

Unit tests cover wrong Java major, bad architecture, invalid manual selection
not being persisted, malformed and unsafe runtime paths, checksum failure,
cleanup, cancellation, a hung Java executable, merged loader metadata, and the
120-second total preparation deadline. The timeout returns a structured error;
the Vue dialog offers Retry, Cancel, an immediate native Java file picker, and
the official Eclipse Adoptium Temurin release page parameterized by required
major, OS and architecture. Real native picker tests accepted Java 21 and
rejected Java 17 for a Java 21 instance. The rejected executable was not saved.

An isolated Tauri user profile was used for the actual UI flow: create offline
account `NCreateQA`, create Minecraft 1.16.5 instance, install it, launch it
through the Library button to the visible Minecraft main menu, and stop it
through the Library button. The game-window capture is in ignored local
`verification/stable-1.0.0/actual-minecraft-game-window.png`.

## Desktop visual verification

Real Tauri WebView pages Home, Library, Content, Accounts and Settings were
inspected at 860×620 and 1240×820, including scrolling, text overflow, long
Russian labels, keyboard Tab/Escape, hover, resizing, account switcher, account
dialogs, About, Java recovery and native Java picker. No horizontal overflow or
clipped controls remained in the checked states. The checked page captures are
in [`docs/screenshots/stable-1.0.0`](../../screenshots/stable-1.0.0/). Additional
Java, update, error, empty-state and progress captures are in ignored local
`verification/stable-1.0.0/`. Update and empty-state captures used UI state
simulation for visual inspection; they are not evidence of a successful update.
Actual install and launch/stop state was checked separately through the Tauri
application.

## Platform and external-service limits

CI run [37458967450](https://github.com/Yozekkk/ncreate-launcher/actions/runs/37458967450)
completed successfully at commit `cbe06249034b2924313e7f8da9102def49cecf9b`.
Both Linux and Windows jobs passed frontend build, Rust check/clippy/tests,
release metadata and boundary checks, executable platform Java probes, installer
builds and artifact uploads. The Windows probe executed Java 21 from a spaced
`Program Files/Eclipse Adoptium` path, and the Windows job built NSIS. The CI
artifacts are named `desktop-Linux-cbe06249034b2924313e7f8da9102def49cecf9b`
and `desktop-Windows-cbe06249034b2924313e7f8da9102def49cecf9b`.
Physical Windows GUI/runtime verification is unavailable on this Linux machine:
**Windows CI/build PASS; physical runtime unverified**.
Microsoft and Ely.by authentication need real credentials, the NCreate server
join needs a live server/session, and installing an actual signed self-update
needs a published second release. Those paths were not marked PASS. The app's
offline account and signed artifact checks were performed without credentials.
