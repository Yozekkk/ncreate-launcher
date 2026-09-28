# Development, packaging and updates

## Linux development

Node.js 22.12+ (or newer supported LTS), pnpm 10.30.3 and current Rust stable
are required. Tauri v2 requires GTK3/WebKitGTK 4.1 development libraries.
On EndeavourOS/Arch, install the native prerequisites:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 openssl librsvg patchelf libappindicator-gtk3 fuse2
```

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm app:dev
```

This starts the actual Tauri application and its local Vite development server.
It does not start the Modrinth website or a marketplace server. A working user
session keyring is needed to save Microsoft credentials securely on Linux;
offline profiles do not require a keyring or an internet connection.

## Local checks and production Linux build

```sh
pnpm prepr:frontend:app
pnpm build
cargo fmt --all --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
pnpm app:build --bundles appimage,deb
```

Tauri writes Linux output under `target/release/bundle/appimage/` and
`target/release/bundle/deb/`. `scripts/build.mjs` normalizes local filenames to
the public NCreate names below after a successful Tauri build. On a
machine without FUSE, an AppImage can be launched with
`APPIMAGE_EXTRACT_AND_RUN=1 /path/to/the.AppImage`.

On Arch/EndeavourOS, linuxdeploy's bundled old `strip` can reject the modern
RELR sections in system libraries. `scripts/build.mjs` defaults `NO_STRIP=true`
on Linux to preserve those dependency ELF files; Cargo's release profile still
strips the application binary. This follows the upstream linuxdeploy reports
[RELR compatibility issue #272](https://github.com/linuxdeploy/linuxdeploy/issues/272)
and [NO_STRIP support #72](https://github.com/linuxdeploy/linuxdeploy/issues/72).
The local build's glibc baseline is the current host's baseline; it does not
prove portability to old Ubuntu. The Ubuntu 22.04 CI build is intended for
distribution with an older baseline, subject to actual build/runtime checks.

The initial Windows build is native x64 on a Windows GitHub runner:

```sh
pnpm install --frozen-lockfile
pnpm app:build --bundles nsis
```

Windows output is under `target/release/bundle/nsis/`. NSIS is configured for
current-user installation, Russian/English installer UI and the NCreate icon.
Windows installers need separate Windows runtime/installation verification;
a Linux production build cannot prove their behavior.

## CI and release artifacts

`.github/workflows/check.yml` checks main pushes and pull requests on Linux x64
and Windows x64. It installs locked dependencies, runs frontend lint/typecheck,
builds the frontend, checks Rust formatting/check/clippy/tests, then produces
desktop bundles. Rust clippy warnings fail the job. Linux is built on Ubuntu
22.04 to avoid unnecessarily raising its glibc floor; the actual runtime still
requires compatible WebKitGTK and system libraries. See the official
[Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux).

`.github/workflows/release.yml` runs for `v*` tags. The tag must match the
version in all root/app/frontend package metadata, `apps/app/tauri.conf.json`
and every Cargo workspace package; mismatches fail before the build starts.
Both platform jobs must pass before the publish job runs. It publishes:

- `NCreate-Launcher-Setup-x.x.x.exe` — Windows x64 NSIS.
- `NCreate-Launcher-x.x.x.AppImage` — Linux x64.
- `NCreate-Launcher-x.x.x-amd64.deb` — Linux x64 Debian package.
- `NCreate-Launcher-vx.x.x-source.tar.gz` — exact tagged source including license notices and build configuration.
- `SHA256SUMS.txt` — hashes of the attached installers and source archive.

The source archive is included alongside the GPLv3 binary distribution.
JavaScript and Cargo lockfiles are part of the repository. There is no source
submodule or private source dependency requiring extra credentials.

The workflow uses only the automatically supplied repository `GITHUB_TOKEN`
for release publication. It does not need signing secrets for stage one.
Installers are currently unsigned; Authenticode signing is a separate future
task requiring a real certificate/service. Automatic updates are disabled.
See [Tauri's GitHub distribution guide](https://v2.tauri.app/distribute/pipelines/github/)
and [Windows installer guide](https://v2.tauri.app/distribute/windows-installer/).

## Publishing a version

Before creating a tag, update the app and workspace versions, the planned package names in both `README.md` and `README.en.md`, and `docs/releases/vx.x.x.md`. Keep direct download links on the last verified public release until the new installers exist. The notes file's first heading becomes the release title. Run the documented checks and wait for the main CI build to pass, then tag that verified commit. Once all release assets are verified and published, update both README download tables together to point at the new files.

The workflow creates a draft and publishes it only after both platform jobs succeed and every required asset is present. Re-running it leaves an existing published release unchanged. An existing draft is resumed only when same-name assets match their SHA-256 hashes; different content stops publication without overwriting assets.

Version `v0.1.0` has **Beta** in its title and notes but uses GitHub's regular release channel so `releases/latest` resolves to its downloads. Hyphenated prerelease tags use GitHub's prerelease channel instead. Updating the release version also requires updating README asset links; the general latest-release link stays unchanged.

The initial release uses installers from successful Desktop checks run [36319792086](https://github.com/Yozekkk/ncreate-launcher/actions/runs/36319792086), built at `22b5c7f8311d5b8b6d0d783af871caf8df016250`. Its tag and accompanying source archive refer to that exact commit. Later presentation commits do not change those published binaries.

## Safe updater plan

Stage one deliberately does not register `tauri-plugin-updater`, grant updater
IPC permissions or configure update endpoints. `createUpdaterArtifacts` is
not enabled. No Modrinth updater URL, signing key, production configuration
or release overlay is retained. The Settings UI must explain that automatic
updates are unavailable until signed NCreate releases exist.

To enable updates later, complete all of these steps together:

1. Generate a NCreate-only updater signing key using the Tauri signer. Keep the private key outside Git and back it up securely.
2. Add GitHub Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Put only the public verification key in the application configuration.
3. Add the official v2 updater plugin and narrowly scoped updater permission. Set `bundle.createUpdaterArtifacts` and pass the secrets only to the build job.
4. Generate and publish signed update bundles, signatures and updater metadata. Use only `https://github.com/Yozekkk/ncreate-launcher/releases/latest/download/latest.json` as the planned public manifest destination; never use the original Modrinth URL.
5. Adapt artifact naming and manifest URLs together. The current stage-one workflow does not generate `latest.json`; simply adding an endpoint is insufficient.
6. Test a real version upgrade on Windows and Linux, signature rejection, a network failure and a cancellation. Only then enable automatic check controls.

Updater signatures and Windows Authenticode signing solve different problems.
No placeholder key, fake secret or unsigned automatic update is acceptable.
Reference: [official Tauri updater documentation](https://v2.tauri.app/plugin/updater/).

## Verification evidence

The build wrapper invokes `pnpm.cmd` through the Windows command shell because
Windows batch commands cannot be spawned directly like Unix executables. This
path has been reviewed against Node's subprocess behavior; only an actual
Windows runner build proves NSIS success. Linux filenames are normalized by
the same wrapper. Use the documented build arguments from CI; the wrapper is
a developer command, not an untrusted user-input interface. Reference:
[Node.js Windows batch subprocess documentation](https://nodejs.org/api/child_process.html#spawning-bat-and-cmd-files-on-windows).

The presence of workflow YAML is a prepared pipeline, not a passed GitHub run.
Record the actual local checks, artifact path, desktop smoke test and remote
workflow URL in the final report. An external Microsoft browser authentication
step and unavailable signing credentials must be reported separately from
offline-account and Linux desktop results.

Standalone debug restart was observed to replace the process and retain its
saved account snapshot. `tauri dev` supervises the app and terminates its restart
child; this development-wrapper behavior is distinct from testing restart in
the production executable/AppImage. Microsoft sign-in reached the official
authentication page, but completing/saving an authenticated account requires
the account owner to log in interactively.
