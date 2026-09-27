# Stage-one requirements and evidence

This records observed stage-one evidence on 2026-09-26 and final review on 2026-09-27. Linux packaging and production smoke passed; remote
CI/publication status is recorded separately. Source code existence alone cannot justify
`PASS`; partial rows name the unverified remainder.

Status vocabulary: `PASS` means the stated behavior was verified; `PARTIAL`
means a concrete portion works and the remaining gap is listed; `BLOCKED`
names an external dependency; `NOT IMPLEMENTED` means functionality is absent;
`PENDING` means implementation/verification is still in progress.

| Original section | Acceptance checks | Status and evidence |
| --- | --- | --- |
| 1. GitHub and structure | Audit current upstream main and licenses; standalone desktop boundary; preserve notices; separate `Yozekkk/ncreate-launcher`; do not modify upstream/site | PASS — upstream main/licenses audited; original source/site untouched; separate public repository created and main pushed. Windows/Linux CI succeeded in run 36256748738. |
| 2. Branding | Product/window/binary/package/installer/desktop names; unique identifier; `ncreate://`; error/loading/About/README text; isolated directories | PARTIAL — real Linux window/title and NCreate interface verified; metadata and directory isolation inspected; warm ncreate://settings routing changed the actual page with one process. Linux AppImage/deb names, native title/class/icon and packaged desktop entry verified; Windows runtime pending. |
| 3. NCreate design | Official site logo/assets; dark orange/amber identity; readable premium dashboard; motion/rounded surfaces | PASS — official NCreate assets used; real X11 desktop screenshots inspected and Home refined; 860×620 layout checked. |
| 4. Home | Three large Minimal/Standard/Ultra cards with Russian text and recommended Standard badge | PASS — real desktop Home shows Minimal/Standard/Ultra and recommended Standard; small-window scroll reaches all buttons. |
| 5. Unavailable editions | Disabled Soon buttons/tooltips; typed `minimal/standard/ultra` models; no hidden install or launch | PASS — all three Soon buttons remain disabled; typed models have null manifests; no native launch/install command exposed. The future typed adapter requires both a manifest and handler; six availability/dispatch/cancellation/error tests pass. |
| 6. Removed features | No browse/store/import/create instance/hosting/friends/news/monetization/Modrinth login route or reachable UI | PASS — standalone route/source audit and desktop navigation expose only Home/Accounts/Settings; excluded upstream product clients absent. |
| 7. Microsoft | Reuse upstream Minecraft auth; browser OAuth; saved nickname/UUID/avatar; activate/remove; secure tokens; no secret logs | PARTIAL — upstream OAuth/Xbox/XSTS flow reaches official Microsoft sign-in; callback/state tests and native credential-store roundtrip pass. Account-owner completion, profile save/refresh/remove remain BLOCKED on interactive credentials. |
| 8. Offline | Nickname-only creation; deterministic offline UUID; avatar/type/active state; persists across restart | PASS — two offline profiles created; activate/rename updates deterministic UUID; snapshot persisted after restart; no password required. |
| 9. Ely.by | Official skin API; nickname lookup without password; face/skin and local fallback; provider abstraction; future auth boundary | PASS — real ErickSkrauch lookup follows Ely.by redirect, valid PNG/head rendered; nonexistent skin yields local fallback; host/path redirect tests pass. Full Ely.by authentication intentionally excluded. |
| 10. Account cabinet | Click profile; nickname/type/UUID/skin/status; activate/delete; Microsoft refresh; offline rename UUID semantics | PARTIAL — offline account cabinet/skin/type/UUID/active controls, activation and rename verified. Authenticated Microsoft refresh awaits account-owner login. |
| 11. Sidebar | Home/Accounts/Settings only; active nickname and avatar in footer; navigation works | PASS — sidebar navigation and active account footer verified in real desktop and 860×620 window. |
| 12. Settings | Launcher language/theme; autostart if feasible; safe updater control; future RAM/Java/game directory; animation/blur/reduced motion; persistence | PARTIAL — Settings rendered/scroll-tested with future Minecraft fields, theme/motion/blur and disabled updater. Russian is the available first-stage language; startup behavior across a real system login not tested. |
| 13. Installer | Real Tauri app; Linux dev; Linux AppImage and preferably deb; Windows x64 NSIS names; no macOS dependency | PARTIAL — Linux native desktop dev works; final Linux AppImage (154 MiB) and deb (5.8 MiB) built, normalized names verified, actual AppImage UI/persistence/restart inspected. Windows x64 NSIS successfully built and downloaded from real GitHub Actions. Windows installation/runtime remains untested on this Linux host. |
| 14. Actions | Push/PR dependency install/lint/typecheck/Rust/build; v-tag Windows and Linux bundles; no committed secrets | PASS for pipeline — actionlint/version guard and actual push workflow passed on Ubuntu 22.04 and Windows, producing all three installers. Tag publication is configured; no release tag was published. |
| 15. Updater | No Modrinth endpoint; safe disabled initial state; future signing/public config/secrets documented | PASS — updater plugin/endpoints/capabilities absent; control safely disabled; future NCreate-only signing/endpoint steps documented. |
| 16. Network | Audit production API hosts; no unwanted startup calls; explain every retained Modrinth destination | PASS — source destination audit records removed services and inactive provenance strings; skin redirect allowlist tested. No full packet capture claimed. |
| 17. Privacy | No Modrinth/PostHog/Sentry telemetry; local diagnostics; no passwords/tokens logged | PASS — telemetry dependencies/product clients absent; token redaction regression test and OS credential-store roundtrip verified; no plaintext token fallback. |
| 18. Data directories | Separate NCreate Linux/Windows data; never read/migrate/write/delete existing Modrinth data | PASS — separate NCreate data store observed; persistence/restart verified; original Modrinth and NCreate site data untouched. |
| 19. Icons | Official best-quality logo; PNG/ICO/ICNS; window/installer icon; no Modrinth artwork | PARTIAL — official NCreate assets and PNG/ICO/ICNS generated; desktop UI icon inspected. Windows installer icon runtime verification pending. |
| 20. UX | Loading/empty/error/network/auth/skin states; focus/keyboard/scrollbar/hover; resize; no jumps/clipping; light 150–250 ms motion | PARTIAL — real screenshots cover empty/validation/auth progress/skins/fallback and 860×620 resize/scroll; offline form autofocus/Tab/Escape verified. Network banner used simulated navigator offline state, not physical disconnect. Full Microsoft completion/platform UX unavailable. |
| 21. Russian | Natural Russian labels; architecture allows future English | PASS — Russian screens inspected; messages isolated in locales/ru with future locale boundary. English not implemented in stage one. |
| 22. Prohibitions | No Electron/site replacement/brand tint only; no dummy manifests/arbitrary packs; no secrets; notices retained; site untouched | PASS — Vue/Tauri desktop adaptation; unavailable editions/no dummy manifests; no Electron/marketplace/secrets; license notices retained; site pristine. |
| 23. Workflow | Audit before adaptation; launch early; accounts/skins/settings/build/runtime; logical verified commits; push after verification | PASS — audit, real launch/refinement and root-cause debugging performed; verified auth, desktop, CI, LF correction and edition adapter commits created. Main source pushed after desktop checks. |
| 24. Desktop smoke | Linux real window title/icon; editions/sidebar; offline create/persist/switch; skin/fallback; settings; restart; no console panic; separate data; OAuth boundary | PARTIAL — real X11 dev desktop smoke, offline profiles/skins/sidebar/settings and standalone debug restart pass. production AppImage Home/accounts/settings and restart inspected; cold ncreate://settings opens Settings. Full Microsoft completion externally blocked. |
| 25. Definition of Done | Real desktop; complete design; unavailable editions; saved offline and Microsoft flow; Ely skins; settings; installers/CI; tested Linux; working GitHub main | PARTIAL — working native desktop/accounts/skins/settings verified. Linux AppImage/deb and production smoke pass. Windows installer build and GitHub main source pass. Full Microsoft profile completion remains externally blocked by interactive user authentication; Windows installation/runtime is unverified. |
| 26. Final report | Reuse/removal/files/accounts/auth/skins/models/endpoints/data/artifacts/check results; pushed SHA/repo; exact dev/build commands/artifact; disclose gaps | PASS — docs/VERIFICATION.md and README record source reuse, account/skin boundaries, editions, endpoints/data, checks, artifact paths and exact commands. Final response identifies the pushed main SHA and remaining external gaps. |

## Required command evidence

| Check | Actual command | Result / evidence |
| --- | --- | --- |
| Locked dependency install | `pnpm install --frozen-lockfile` | PASS — final locked install log, pnpm 10.30.3 |
| Frontend lint and TypeScript | `pnpm prepr:frontend:app` | PASS — vue-tsc, ESLint and Prettier final run |
| Frontend build | `pnpm build` | PASS — Vite 8.3.1 final production build |
| Rust formatting | `cargo fmt --all --check` | PASS — final independent formatting check after redirect test correction |
| Rust check | `cargo check --locked --workspace --all-targets` | PASS — final 2026-09-26 run includes skin redirect changes |
| Rust clippy | `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS — final 2026-09-26 run includes skin redirect changes |
| Rust tests | `cargo test --locked --workspace` | PASS — final run: 2 library + 13 shell tests, including redirect allowlist |
| Desktop development launch | `pnpm app:dev` | PASS — native Tauri X11 window inspected, diagnostics and screenshots |
| Linux production bundles | `pnpm app:build --bundles appimage,deb` | PASS — final AppImage and deb build completed after native patchelf installation and NO_STRIP compatibility fix |
| Production app smoke | Launch actual built binary/AppImage | PASS — actual AppImage on Linux, persisted Ely.by account, Home/accounts/settings, restart, cold link; no release bridge |
| Windows NSIS | Windows workflow run + installer verification | PASS build — real Windows CI produced NCreate-Launcher-Setup-0.1.0.exe; downloaded PE/NSIS archive inspected. PARTIAL installation/runtime — no Windows desktop available locally |
| GitHub source | Repository URL + pushed main SHA | PASS — https://github.com/Yozekkk/ncreate-launcher, main; exact final SHA is reported with git rev-parse HEAD |

## Required visual evidence

Each screen needs a screenshot inspected from the real desktop WebView,
interaction checks and correction/recheck as needed. Do not substitute a
frontend compile result or screenshot filename for actually viewing the image.

| Screen / interaction | Screenshot and inspected checks | Result |
| --- | --- | --- |
| Home | `home-refined.png`, `home-small.png`, `home-small-bottom.png`; inspected layout, recommendation, disabled buttons and 860×620 scrolling | PASS |
| Accounts | `accounts-empty.png`, `accounts-ready.png`, `accounts-small.png`; real empty/ready and small-window state | PASS for observed screens; Microsoft saved card pending |
| Add Offline Account | `offline-validation.png`, `offline-small-final.png`; validation/create, autofocus/Tab/Escape and small-window fit | PASS |
| Microsoft Account flow | `microsoft-progress.png`, `microsoft-signin.png`; official sign-in reached and inspected | PARTIAL — interactive account-owner login BLOCKED |
| Settings | `settings-final.png`, `settings-small.png`, `settings-bottom.png`; real desktop and 860×620 scrolling | PASS for inspected screens; system-login autostart untested |

## External blockers and excluded work

No actual Minecraft edition manifests, downloads, installs or launches belong
to this stage. Microsoft account completion may require the account owner's
interactive browser authentication. Updater signing and Windows Authenticode
require real private keys/certificates; signing is not simulated. GitHub
authentication and remote CI outcomes must be recorded as observed.

## Workflow configuration verification

Both workflow files passed `actionlint` v1.7.12 after the packaging review.
The exact release version guard ran locally: `v0.1.0` accepted all current
JavaScript/Tauri/Cargo workspace versions; `v9.9.9` and `vinvalid` were rejected.
Actual Windows and Linux jobs also succeeded in [run 36256748738](https://github.com/Yozekkk/ncreate-launcher/actions/runs/36256748738), on source e307fd3cf2a09a1ff689db177e06ced4581f9f0c. Its NSIS/AppImage/deb artifacts were downloaded and inspected. Later adapter changes add six tests and retain the same unavailable default editions; the latest main workflow result is linked in the final report.

## Runtime and packaging limitations

Screenshots above live in local `verification/`; selected reviewed screens are committed in `docs/screenshots/`. they were inspected
by the implementation agent from the actual Tauri WebView, not a browser-only
mock. `account-ely.png` and `account-fallback-final.png` record actual Ely.by
preview and missing-skin fallback. Microsoft completion requires the account
owner's browser authentication; no credentials were supplied or simulated.

Standalone debug `request_restart` produced a new process ID and restored the
same saved snapshot. Under `tauri dev`, the supervising CLI terminates the
restart child; packaged AppImage restart was verified separately with a new process ID, preserved account/settings and successful reopened screens. The native
credential store completed a write/read/delete roundtrip without logging a
token. Rust tests also cover callback origin/state and secret-safe errors.

Linux packaging first failed because linuxdeploy's bundled old `strip` cannot
read the host's RELR ELF sections. The build wrapper now defaults Linux
`NO_STRIP=true`; Rust's release profile still strips its own executable. The
next production run passed that phase but failed when the gstreamer plugin
could not locate patchelf. The official Arch patchelf package was installed
and the final packaging rerun succeeded. Actual AppImage screens and restart were inspected.
The local Arch/EndeavourOS build depends on the host's current glibc baseline;
it is not evidence of compatibility with older Ubuntu. Ubuntu 22.04 release
CI is intended to produce binaries with an older baseline.

Native offline form autofocus and Tab focus were observed; Escape closed the
form. A simulated `navigator` offline condition displayed the network banner
without breaking the page; the machine was not physically disconnected.
A warm `ncreate://settings` binary argument changed the actual settings route
and left a single application process. Cold `ncreate://settings` AppImage launch was also verified after adding the initial-route IPC command. Final Microsoft exchange disables cancellation once browser authentication is complete.

## Final review limits

- PASS: Linux production desktop, unavailable editions, offline create/persist/switch/rename/delete, real Ely.by image and fallback, settings persistence, native restart, warm/cold NCreate routing and production bridge exclusion.
- PASS: locked dependency install, frontend typecheck/lint/format/build, six adapter tests, Rust fmt/check/clippy and 15 Rust tests; real Windows/Linux CI installer builds.
- PARTIAL / BLOCKED: Microsoft initiation and official browser page work; full authenticated save/refresh needs the account owner to finish authentication.
- PARTIAL: Windows NSIS is built, but installation and GUI interaction on Windows have not been verified here; system-login autostart also remains untested.
- NOT IMPLEMENTED by stage-one scope: Minecraft downloads/launches, full Ely.by account auth, English UI and signed automatic updates. Updater/Authenticode activation requires real keys/certificates.
- Local test profiles were removed through the application and theme/motion defaults restored.
