# NCreate Launcher

Standalone GPLv3 desktop adaptation of Modrinth App. Upstream provenance and
preserved license notices are in NOTICE.md, COPYING.md and docs/UPSTREAM-AUDIT.md.

## Architecture

- apps/app: Tauri v2 native shell, isolated local storage, skins and IPC.
- apps/app-frontend: Vue 3 desktop interface: Home, Library, Content, Accounts and Settings.
- packages/app-lib: extracted upstream Microsoft/Minecraft authentication and Ely.by providers.
- packages/launcher-core: upstream-backed game/content installation, managed editions and transactions.
- packages/app-lib/src/upstream: inactive source comparison; never compile it.

Use tabs for indentation. Keep comments explanatory and avoid heading comments.
Keep changes focused on NCreate; keep public content browsing separate from Hosting, advertising, social systems and telemetry.

## Checks and launch

Run from the root:

```sh
pnpm install --frozen-lockfile
pnpm prepr:frontend:app
pnpm build
cargo fmt --package ncreate-launcher --package ncreate-app-lib --package ncreate-launcher-core --check
cargo check --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets
cargo clippy --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets --no-deps -- -D warnings
cargo test --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core
pnpm app:dev
pnpm app:build --bundles appimage,deb
```

Use real rendered desktop verification for interface changes. Keep the development
bridge behind debug_assertions; release builds must not expose it. Never print,
commit or return credentials through IPC. Microsoft tokens belong in the native
credential store. Do not read or migrate the original application's user data.

Official editions remain unavailable until real NCreate manifests are configured. Custom
instances support installation and launch through the native engine. Updater
endpoints and signing are disabled until real NCreate signing keys exist. Preserve
all GPL notices when redistributing source or binaries. Check docs/REQUIREMENTS.md
for observed results and external authentication/signing limits.
