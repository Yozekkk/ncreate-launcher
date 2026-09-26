# Upstream audit

Audit source: `https://github.com/modrinth/code`, branch `main`, commit
`0e817be41cebdb0e140866e87e727c2101fe0e6b`, retrieved 2026-09-26.
The upstream checkout is read only; NCreate is a separate repository.

## Files read before adaptation

- Root `AGENTS.md` and `COPYING.md`.
- `apps/app/COPYING.md`, `apps/app/LICENSE`, `apps/app/tauri.conf.json` and `tauri-release.conf.json`.
- `packages/app-lib/COPYING.md` and `src/state/minecraft_auth.rs`.
- Desktop shell entry point, frontend app/router/configuration and telemetry helpers.

There is no upstream `apps/app/AGENTS.md`. Root instructions therefore apply.
The retained notices and GPL-3.0-only license documents describe the upstream
origin. Names in these legal documents are attribution, not product branding.

## Standalone boundary

Upstream desktop already separates the Vue frontend (`apps/app-frontend`),
Tauri shell (`apps/app`) and Rust application library (`packages/app-lib`).
NCreate preserves these boundaries, Vue 3, Tauri v2, Rust and SQLite persistence.
The Minecraft Microsoft authentication protocol is adapted from the upstream
`packages/app-lib/src/state/minecraft_auth.rs`; this includes Microsoft OAuth,
Xbox device/SISU authentication, XSTS, Minecraft token exchange, entitlements
and Minecraft profile retrieval. It does not request Microsoft passwords.

Stage one includes only these three application packages. The full upstream
website, Labrinth API server, marketplace API client, hosting, advertising,
friends, notifications, payment integrations and analytics packages are not
part of the standalone workspace. The large upstream generic game-instance
manager and modpack download/launch machinery are excluded at this stage:
there are no real NCreate manifests and no game launch command. This is a
focused desktop adaptation with an upstream authentication extraction, not a
claim that the complete upstream backend was retained unchanged.

## Product adaptation

The frontend navigation is limited to Home, Accounts and Settings. Edition
identities are `minimal`, `standard` and `ultra`; all three remain unavailable.
No arbitrary mod or modpack import route is exposed. The original product UI
is replaced with the NCreate dashboard and brand materials obtained read only
from `Yozekkk/ncreate-site`.

Tauri metadata uses `NCreate Launcher`, `com.ncreate.launcher` and `ncreate://`.
The former `mrpack` association, `modrinth://`, broad remote CSP rules and
advertising capabilities are excluded. User data is isolated from Modrinth.
Updater support is disabled; upstream updater configuration and public key
are not used. See [Network audit](NETWORK.md) and [Release guide](RELEASE.md).

## License and distribution records

Root and desktop/backend GPL license and copying notices are preserved.
Release workflow attaches a source archive of the exact tagged commit beside
the binary packages, along with SHA256 checksums. Root license is included as
a packaged resource. Modrinth logos and branding assets are not distributed.
This records the distribution measures used by the project; it is not a legal
opinion about every dependency's license.

## Verification boundary

Source inspection alone does not prove runtime behavior or installer success.
The requirements record and final verification report must distinguish checked
behavior from code present, external authentication and platform checks that
have not run. Windows installers require the Windows CI runner; completing a
Microsoft login requires an account owner to authenticate interactively.
