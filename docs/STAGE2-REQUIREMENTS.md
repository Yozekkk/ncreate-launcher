# Stage 2 requirements and evidence ledger

The source of truth is remote main. This stage restores custom Minecraft instances and public content support while keeping NCreate branding, isolated data and credential storage. A real authenticated account is never inferred from synthetic tests. No public release is authorized by this stage.

## Required deliverables

1. Audit current routes, IPC, SQLite, credentials, auth, skins, editions, settings, capabilities, CSP, filesystem, network, updater, tests and CI; audit current upstream installation, loaders, Java, launch, instances and content. Record upstream reuse/adaptations/exclusions in STAGE2-UPSTREAM-AUDIT.md.
2. Read and apply installed workflow, Rust, design, visual-verification and relevant Tauri skills. Missing optional skills do not block unrelated work.
3. Microsoft tests: state, PKCE, callback origin/state, malformed/cancelled/expired flows, invalid OAuth, XSTS errors, absent entitlement/profile, refresh/expiry, native credential save/read/delete, logout/removal/multiple accounts. Never log credentials, authorization codes or cookies.
4. Automatically verify official Microsoft domain/TLS/redirect, callback interception/listener, deep links, state and cancellation. Never access passwords, browser cookies, password managers or bypass MFA.
5. Deterministic integration harness runs the actual Microsoft OAuth/SISU/XSTS/Minecraft/entitlements/profile implementation through login begin, callback, finish, persisted account, restart, refresh and logout. Synthetic success is distinct from real-account success.
6. Ely.by skins: real live existing and absent nickname requests; valid/malformed/oversized PNG, restricted redirects, timeout/offline, cache hit/invalidation, local fallback.
7. Authentic Ely.by account provider: officially documented OAuth where suitable, configurable external client prerequisite, legacy fallback only with transient erased password and native credential tokens. Login, selected profile, refresh/validate/logout, persistence/restart/multiple accounts.
8. Ely.by protocol endpoint/error/mock/persistence/native-vault checks. Real live successful authentication requires owner-supplied credentials; otherwise real-account status BLOCKED.
9. Separate AccountProvider (Microsoft, ElyBy, Offline), GameIdentity, SkinProvider and CredentialReference. Offline Ely.by skin does not imply Ely.by authentication.
10. Library: icon, name, Minecraft, loader, mod count, last played, status, play and menu; create/open/rename/icon/delete/folder/duplicate; export if stable upstream permits.
11. Create Vanilla/Fabric/Forge/NeoForge/Quilt instances using official/upstream metadata, not invented loader versions.
12. Safe .mrpack import: reject traversal, absolute paths, symlinks, oversize, malformed manifest, unknown external sources and hash mismatch.
13. Separate NCreate-styled content route Mods and Modpacks.
14. Official public Modrinth API/client, no scraping. Resource packs/shaders optional.
15. Search query/category/Minecraft/loader/sort/pagination; project icon/title/author/description/downloads/versions/loader/update date.
16. Compatible target instance selection, compatible mod version, dependency resolution, verified download, correct path and saved metadata; block incompatible loaders.
17. Per-instance Mods with version/enabled/update state and enable/disable/update/remove/project/check actions. Disable must be reversible.
18. Compatible per-mod and bulk updates account for Minecraft, loader, channel and dependencies; preserve rollback metadata/snapshot.
19. Modpack browsing/detail/version selection and install as new instance using adapted upstream flow.
20. OfficialEdition remains separate from custom instances.
21. Versioned official manifest schema with identity/version/Minecraft/loader/files/Java/memory/launch/server/channel policies.
22. Every manifest file has relative path, source URL, SHA-256, size, required/optional and update policy; every download verified.
23. Official update engine: remote/local diff added/changed/removed, update byte count, changed files only, hash check, atomic/rollback apply and new manifest persistence.
24. Never destroy user saves/screenshots/logs/options/custom resource packs/configs/mods; only explicit managed files are update-owned.
25. Transaction staging/download/hash/apply/journal rollback and crash recovery.
26. Default stable channel; beta only manually enabled in Settings, dev optional.
27. ManifestProvider with centrally configured HTTPS GitHub/static/CDN source; no local machine paths in production config.
28. Shared download manager for editions/Minecraft/loaders/mods/modpacks with progress/bytes/speed/cancel/retry/concurrency/timeout/hash/temp files. Safe resume optional.
29. HTTPS/restricted redirects/bounds/no file URLs/no shell commands/sanitized filenames/traversal prevention.
30. Real Minecraft version/client/libraries/assets/natives/loader installation; no uncontrolled installer shell scripts.
31. Version-aware Java discovery/validation/custom path/actionable errors; reuse upstream runtime management if independent of proprietary endpoints.
32. Custom-instance real Play uses backend-selected Microsoft/Ely.by/offline identity and actual process arguments; no unsupported server compatibility claim.
33. Official Minimal/Standard/Ultra keep Coming soon without production manifests. Fixture packs belong only in tests/dev.
34. Sidebar Home/Library/Content/Accounts/Settings and active account.
35. Preserve NCreate logo/orange/dark/typography/spacing/cards/animations; no green product redesign.
36. Content source attribution can say Powered by Modrinth; preserve GPL attribution without pretending to be official Modrinth App.
37. Exclude Hosting/Friends/ads/Plus/analytics/PostHog/Sentry/monetization/payout/organizations/notifications/social/commercial services.
38. No Modrinth account login for public browsing/download.
39. Transactional migrations preserve v0.1.0 accounts/settings and add provider/instances/content/local-manifest/update-state tables; test original fixture.
40. Back up migration test database; production migration transactional.
41. Update NETWORK.md with Microsoft/Xbox/Minecraft/Mojang/Ely.by/Modrinth/official manifest/loader endpoints and exclusions.
42. Unit tests: manifests/diff/hash/dependencies/compatibility/paths/traversal/interrupted recovery/migrations/account serialization.
43. Integration fixtures: Microsoft success/failures, Ely.by failures, search/version/download, mrpack, instance creation, official install/update v1-v2/rollback. No huge downloads per CI run.
44. Live read-only Modrinth known-project search/versions/compatibility; small public verified download if reasonable.
45. Real Linux desktop: Home, empty Library, Create, search/detail, Accounts, Ely.by skin, Settings, resizing, restart.
46. Inspect screenshots of Library/Create/Mods/Modpacks/Instance Mods/Update dialog, including clipping/overflow/loading/empty/error/disabled/hover/focus.
47. Check idle CPU/polling/rendering; no busy loop or GPU-heavy pervasive blur.
48. Windows/Linux CI: frontend build/types/lint/format, Rust fmt/check/clippy, unit/integration tests without private credentials; no secrets.
49. Prepare version bump and notes; do not create a public release. Commit/push main after checks and verify CI.
50. Safely fast-forward full Documents clone after successful push; preserve user files.
51. Separate Microsoft flow/mock/real and Ely.by skins/protocol/real evidence.
52. Final statuses for Microsoft flow/mock/real, Ely.by skins/protocol/real, instances, search, mod install, modpack install, mod update, manifests, updates, rollback, Minecraft install/launch, Linux desktop, Windows CI.
53. Report commit/CI/upstream modules/exclusions/routes/DB version/schema/download/update/instance paths/screenshots/tests/live API/network audit/exact blockers; no unobserved runtime PASS.
54. Implement, build, run, inspect, test, diagnose, fix, rerun, visually inspect, atomic commit, push, CI, resolve failures, final verification. Do not stop after source changes.

## Evidence

Initial main: `5d9656e427e2f002cc5843adf6a95a3c5605215b`, clean checkout. Initial upstream main: `0e817be41cebdb0e140866e87e727c2101fe0e6b`; refreshed to `23ab5cdc331e40712f12b9026979359aec1e404d` on 2026-09-28. The exact observed auth, fixture, live API, desktop and loader results are recorded in [STAGE2-VERIFICATION.md](STAGE2-VERIFICATION.md); screenshots are in `docs/screenshots/stage2`. Microsoft/Ely.by real account success and a published NCreate edition have not been observed. The final CI result and commit SHA are established only after push.
