# Stage 2 upstream audit

Audited on 2026-09-27 and refreshed on 2026-09-28 against `modrinth/code` main at `23ab5cdc331e40712f12b9026979359aec1e404d`. NCreate starts from remote main `5d9656e427e2f002cc5843adf6a95a3c5605215b`, with a clean working tree. The upstream checkout is a read-only reference; no upstream files or user data are changed. The change since the initial upstream audit (`0e817be41cebdb0e140866e87e727c2101fe0e6b`) adds a fallback in `launcher/mod.rs`: when remote loader metadata no longer lists an installed loader, upstream can use the saved local version JSON. That fallback is relevant to installed instances and remains a follow-up for NCreate's `version_info` path.

## Existing NCreate boundary

The v0.1.0 frontend uses hash routes Home, Accounts and Settings in `apps/app-frontend/src/App.vue`. IPC registration is explicit in `apps/app/src/main.rs`; `main` is the only native-capable WebView. The external Microsoft `signin` WebView has no capability. OAuth uses the upstream public Microsoft client, PKCE, Xbox device Proof of Possession, SISU, XSTS, Minecraft token, entitlement and profile requests. The desktop redirect is intercepted in that isolated WebView; it is distinct from the launcher navigation scheme `ncreate://`.

`storage.rs` owns the SQLite metadata/settings database under the isolated NCreate data directory. Native credential storage keeps Microsoft tokens outside SQLite/frontend state. `skins.rs` performs restricted HTTPS skin fetches, bounded PNG validation, cache and fallback. Official edition identities and a guarded installer adapter exist in frontend models; no production edition manifest has been supplied. Settings include appearance, future memory/Java/game-directory preferences and disabled launcher updates.

The baseline CSP permits only local application assets/data images and native IPC. Capabilities expose core window APIs and the three autostart operations only. There is no frontend arbitrary filesystem, HTTP, shell or updater permission. Windows/Linux CI runs locked install, frontend checks/build, Rust format/check/clippy/tests and native installers. Published v0.1.0 installers and tag stay immutable during Stage 2.

## Upstream mechanisms inspected

| Function | Upstream source | Stage 2 adaptation |
|---|---|---|
| Microsoft authentication | `packages/app-lib/src/state/minecraft_auth.rs` | Preserve the extracted protocol; add injectable test endpoints, deterministic success/failure coverage, secret-store lifecycle and backend-only launch sessions. |
| Instance lifecycle/storage | `api/instance/lifecycle.rs`, `state/instances/`, `state/instance_types.rs` | Keep lifecycle semantics and isolated instance IDs; adapt persistence to the existing NCreate database, without the upstream global social/application state. |
| Dependency resolution | `packages/modrinth-content-management/src/install/` | Reuse the independent upstream resolver and compatibility preferences through a public Modrinth metadata adapter. |
| Pack/content diff | `packages/modrinth-content-management/src/diff/` | Reuse configuration/content comparison where applicable; official managed-file ownership adds a separate NCreate transaction journal. |
| Safe pack paths | `packages/path-util/src/lib.rs` | Reuse safe relative UTF-8 Unix path validation and Windows reserved-name rules; add filesystem symlink containment checks. |
| Minecraft metadata | `packages/daedalus/src/minecraft.rs`, `modded.rs` | Reuse metadata types and merging primitives, retaining its MIT license. |
| Minecraft install | `launcher/mod.rs`, `launcher/download.rs` | Adapt client/libraries/assets/natives and loader setup to bounded common downloads, NCreate progress/jobs and shared local runtime storage. |
| Launch rules/arguments | `launcher/mod.rs`, `launcher/args.rs`, `util/platform.rs` | Adapt OS/architecture/rule evaluation, classpath and placeholder expansion; receive identity only from the backend auth boundary. |
| Java | `util/jre.rs`, `api/jre.rs` | Adapt version probing and Java-major selection without importing proprietary service or user-account endpoints. |
| mrpack import | `api/pack/install_mrpack.rs`, `api/pack/install_from.rs` | Preserve the format, client environment rules, hashes and safe override handling; add explicit total/archive bounds and trusted download sources. |
| Content install/update | `api/instance/projects.rs`, `state/content_store/` | Use compatible versions/dependency resolution, reversible disable and transactional content metadata. |
| Export/duplicate | `api/instance/export_mrpack.rs`, instance lifecycle | Adapt local custom-instance operations behind explicit native file selection and managed directory boundaries. |
| Installation recovery | `install/control.rs`, `runner.rs`, `store.rs`, `recovery.rs` | Follow preparation/verification/commit/recovery phases; use a NCreate-owned durable journal and rollback snapshots. |

These are implementation boundaries, not claims that the whole upstream backend was copied unchanged. Independent reused crates retain their source notices; adapted desktop mechanisms retain GPLv3 attribution. Full functionality is claimed only after the corresponding tests and runtime evidence are recorded.

## Deliberate exclusions

No Labrinth server, website server, upstream UI package or commercial marketplace is imported. Hosting, Friends, ads, Plus, analytics/Ariadne, PostHog, Sentry, payout, monetization, organization management, notifications, social synchronization, Modrinth account login and upstream updater are excluded. Public project search, metadata and downloads need no Modrinth account. The content UI identifies Modrinth as its source without adopting its product identity or green visual theme.

NCreate retains its own logo, orange/dark typography, identifier `com.ncreate.launcher`, `ncreate://` links and data directories. Original Modrinth user data is never read, migrated or changed. The official Minimal/Standard/Ultra model stays separate from custom instances; test manifests are never presented as real production editions.

## Integration and evidence

The implementation is split between the existing secure auth package, a new `packages/launcher-core` domain/download/install engine and the NCreate Tauri/frontend adapters. Auth metadata migration precedes engine tables; database schema versions must never be lowered. Import/export/icon paths originate in native dialogs. Long operations have explicit cancellation and terminal states; progress sampling occurs only while work is active.

The detailed requirement ledger is [STAGE2-REQUIREMENTS.md](STAGE2-REQUIREMENTS.md). Observed tests, runtime checks and limitations are in [STAGE2-VERIFICATION.md](STAGE2-VERIFICATION.md). Microsoft/Ely.by real-account completion remains distinct from mocks and requires an account owner; no private credentials are collected by the development workflow.
