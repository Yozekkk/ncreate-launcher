# Development, packaging and releases

## Linux development

Use Node.js 22.12+, pnpm 10.30.3 and current stable Rust. Tauri v2 requires GTK3 and WebKitGTK 4.1 development libraries. On EndeavourOS/Arch:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 openssl librsvg patchelf libappindicator-gtk3 fuse2
pnpm install --frozen-lockfile
pnpm app:dev
```

This starts the desktop application and its local Vite server. A usable system keyring is required for Microsoft and Ely.by token storage. Offline profiles do not require one.

## Checks and local signed build

From the repository root:

```sh
pnpm prepr:frontend:app
pnpm build
node --test scripts/release-assets.test.mjs
cargo fmt --package ncreate-launcher --package ncreate-app-lib --package ncreate-launcher-core --check
cargo check --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets
cargo clippy --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets --no-deps -- -D warnings
cargo test --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core
pnpm app:build --bundles appimage,deb
```

`scripts/build.mjs` loads the local owner-only Tauri signing files when CI signing environment variables are absent. It does not print or copy the private key into the repository. A developer without the production key can run checks and development builds, but cannot create an update artifact accepted by installed production clients.

The resulting Linux files are `target/release/bundle/appimage/NCreate-Launcher-X.Y.Z.AppImage`, its `.sig`, and `target/release/bundle/deb/NCreate-Launcher-X.Y.Z-amd64.deb`. On Windows, `pnpm app:build --bundles nsis` creates the installer and its `.sig`. The NSIS installer is for the current user. It has no commercial Authenticode certificate; that is independent from the Tauri updater signature.

The Linux wrapper sets `NO_STRIP=true` because linuxdeploy's old `strip` cannot read modern Arch RELR sections. Cargo's release profile still strips the application binary. The distributable Linux package is built on Ubuntu 22.04 in CI to keep an older glibc baseline. A local Arch build cannot establish portability to older distributions.

## Automated GitHub release

`.github/workflows/check.yml` tests pushes and pull requests on Linux and Windows, including installers, without signing secrets. `.github/workflows/release.yml` runs only for `v*` tags whose version matches all package, Cargo and Tauri metadata. Both platform jobs must pass before the publish job runs. It requires repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the public verification key is in `apps/app/tauri.conf.json`.

For a new release:

1. Update the application version in every active version definition and add `docs/releases/vX.Y.Z.md`.
2. Run the checks and wait for green `main` CI.
3. Tag that verified commit `vX.Y.Z` and push the tag.
4. Wait for the release workflow to publish the complete draft. Verify the three installers, `latest.json`, `.sig` files, source archive and `SHA256SUMS.txt` on GitHub.
5. Update both README download tables to the new version only after the public downloads are checked.

The workflow signs the Linux AppImage and Windows NSIS installer using the same protected key, creates Tauri static JSON metadata for both platforms, computes SHA-256 checksums, and attaches corresponding GPL source. It validates file existence and hashes before publishing. Re-running a published release does not overwrite assets. A draft with different same-name assets causes a failure instead of silent replacement.

The first heading in the release notes determines the release title. A Beta/Бета title or prerelease SemVer tag sets channel `beta` and marks the GitHub release as prerelease; otherwise it is `stable`. The launcher discovers channel releases through the GitHub Releases JSON API, not HTML or a mutable `releases/latest` shortcut. The published v0.5.0 Beta predates signing and has no `latest.json`, so it is ignored by the updater. Its installed binary has no updater; users must install the first signed release, v0.6.0 Beta, manually once.

See [UPDATES.md](UPDATES.md) for the native update flow, signing location, pack manifests, rollback and verification limits. The [Tauri v2 updater documentation](https://v2.tauri.app/plugin/updater/) defines the signature format and installation behavior.
