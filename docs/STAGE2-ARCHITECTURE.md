# Stage 2 architecture

Stage 2 separates authenticated identity, content sources and official editions.
The application remains `com.ncreate.launcher`, uses `ncreate://` navigation and
never reads the original Modrinth application's data directory.

## Native boundary

The Vue routes are Home, Library, Content, Accounts and Settings. All game and
content operations use the explicitly registered commands in
`apps/app/src/core_commands.rs`. Commands validate the local `main` window.
The remote Microsoft `signin` WebView has no application capabilities.
Native dialogs grant only the selected import/export/icon file; the frontend
has no arbitrary filesystem, HTTP or shell capability. Project descriptions use Markdown with raw HTML disabled and a strict DOMPurify
allowlist. Remote images, navigation links and executable attributes are removed.

`packages/app-lib` retains the upstream Microsoft OAuth/PKCE → Xbox device/SISU
→ XSTS → Minecraft token → entitlement → profile pipeline. Its transport and
credential interfaces permit deterministic fixtures without changing production
endpoints or granting production access to a test server. Real account login is
separate from fixture success. Microsoft passwords are never accepted.

Providers are Microsoft, ElyBy and Offline. `GameIdentity` contains the UUID
and nickname; `SkinProvider` describes appearance; `CredentialReference` refers
to native keyring storage and is not a token. An offline identity with Ely.by
skins remains offline. The legacy Ely.by launcher fallback accepts a transient
password/TOTP request, clears UI fields, and zeroizes backend request secrets;
only issued session credentials enter the native vault. Registered Ely.by OAuth
requires an externally configured client, secret and redirect; no fake client or
secret is embedded. References: [launcher authentication](https://docs.ely.by/en/minecraft-auth.html),
[OAuth](https://docs.ely.by/en/oauth.html), [authlib-injector](https://docs.ely.by/en/authlib-injector.html).

## Persistent layout

The existing SQLite `launcher.db` stores account metadata and settings.
Account migration 2 preserves v0.1.0 rows and adds provider semantics; engine
migration 3 creates instance, installed-content, local-manifest and transaction
records. Migrations are transactional and never lower a future schema version.
Verification fixtures back up an old database before migration and reopen it.
Passwords and access/refresh tokens are excluded from database metadata.

Within the isolated NCreate data directory:

- `instances/<uuid>/`: custom or official game directory, user saves/options and content.
- `minecraft/`: shared client versions, libraries and asset objects.
- `transactions/`: durable installation journals and rollback data.
- `imports/`: temporary validated pack archives.
- `skins/`: bounded public texture cache.
- `manifest-providers.json`: centralized official manifest URLs by stable/beta channel.

Linux uses `~/.local/share/ncreate-launcher`; Windows uses
`%LOCALAPPDATA%\ncreate-launcher`. Tauri window state remains under the separate
`com.ncreate.launcher` configuration directory. Debug verification may use an
isolated task directory; production builds cannot redirect data through a test
configuration.

## Engine and reuse

`packages/launcher-core` adapts desktop lifecycle, download, pack, Java and
Minecraft rules from the audited upstream. Its independent upstream support
crates preserve the Daedalus metadata/merging code, content dependency resolver,
content diff and safe-path utilities. The NCreate database and UI replace
upstream global product/social state; the whole Modrinth backend is not copied.
See [upstream audit](STAGE2-UPSTREAM-AUDIT.md) for exact source boundaries.

Public catalog operations use official Modrinth v2 APIs without a Modrinth
account. Version selection checks Minecraft, loader and release channel.
Content installs verify hashes, resolve required dependencies and retain
metadata. Disabled mods use reversible filenames. Pack import validates the
manifest, client environment, archive paths, source URLs, sizes and hashes.
Import/export and duplication apply only to custom instances. Stable selection
excludes beta and alpha; explicit beta also admits beta releases, never alpha.

`src/download.rs` is the shared bounded download manager. `src/files.rs` owns
safe filesystem paths, staged changes, durable journals, recovery and rollback.
`src/editions.rs` owns the separate official manifest install/update adapter;
`src/minecraft.rs` owns runtime metadata, installation, Java checks and launch.
Java discovery checks the required major version and allows a custom executable;
managed Java downloads are not implemented. Install/import creates instance files;
the separate Minecraft installation action prepares the runtime before Play.
Modpack version replacement is not implemented; compatible individual mod updates
and official NCreate manifest updates are separate operations.
Cancellation and terminal jobs are observable by the frontend without idle
polling while idle. Library status refreshes only while a game process is running. Sensitive launch identity is constructed only in Rust, immediately
before starting Java, and is never serialized into frontend state.

## Official manifests

The versioned schema lives in
`packages/launcher-core/schema/official-edition-v1.schema.json`.
Minimal, Standard and Ultra retain distinct official IDs. Production provider
maps start empty, so all three buttons remain “Скоро”. Fixtures are confined to
tests. Stable is the default; beta requires explicit selection in Settings.

Each managed file specifies safe relative path, HTTPS source, SHA-256, exact
size, required/optional state and update policy. Updates compare manifests,
stage and verify changed bytes, then apply through the durable transaction
journal. User saves, screenshots, logs, options and unowned content are outside
the managed-file set. Conflicting modified files must not be silently replaced.
Crash recovery and rollback restore both files and corresponding metadata.

## Exclusions and release policy

Hosting, Friends, ads, Plus, creator payouts, organization management, social
notifications, Modrinth account login, analytics, PostHog, Sentry and the
Modrinth application updater stay absent. The public loader metadata host is
explicitly documented in [NETWORK.md](NETWORK.md); it is not an updater.

The source version is prepared as 0.2.0. Windows/Linux CI builds native packages
and runs credential-free tests. No Stage 2 public release is published merely
because the code is committed. The existing v0.1.0 tag and assets are immutable.
