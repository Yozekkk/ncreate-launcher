# Microsoft authentication source

The active `src/minecraft_auth.rs` adapts
`packages/app-lib/src/state/minecraft_auth.rs` from Modrinth App's `main`.
The untouched source is preserved in `src/upstream/minecraft_auth.rs` for review;
that file is not compiled and its insecure persistence/logging are not used.
Upstream copyright and GPLv3 remain applicable; see the repository licenses.

The Xbox device Proof of Possession, SISU authentication/authorization, PKCE,
OAuth token exchange/refresh, XSTS, Minecraft token, entitlement and profile
steps retain upstream protocol details and its public Microsoft client ID.
The redirect remains `https://login.live.com/oauth20_desktop.srf`, intercepted
inside an isolated Microsoft sign-in WebView. NCreate's `ncreate://` scheme is
independent of this upstream callback. OAuth state is now checked explicitly.

Removed: global State, SQLite credential persistence, onboarding, Modrinth
services, startup jobs, credential serialization to the WebView, sensitive
tracing arguments, and raw response bodies from errors. HTTPS is mandatory.
Xbox device keys are ephemeral backend memory; Microsoft access and refresh
tokens are native-vault items in service `com.ncreate.launcher` keyed by UUID.
Linux uses Secret Service, Windows Credential Manager, macOS Keychain.
There is no plaintext fallback if secure storage is unavailable.

The shell persists only metadata and keeps active selection separately. Public
IPC-safe results contain UUID, nickname and skin URL. Offline accounts never
enter the Microsoft credential store.

## Skin provider references

Ely.by skins are public and need no password: official
[skin-system documentation](https://docs.ely.by/en/skins-system.html).
`GET https://skinsystem.ely.by/skins/{nickname}.png` returns PNG or 404;
`/textures/{nickname}` returns texture metadata or 204. Names are case-insensitive.
Ely.by may proxy Mojang skins automatically. Runtime PNG decoding, fallback and
cache belong to the launcher shell's skin provider abstraction.
Future account authentication should use the officially recommended
[OAuth flow](https://docs.ely.by/en/minecraft-auth.html) and
`minecraft_server_session` scope. [Authlib-injector](https://docs.ely.by/en/authlib-injector.html)
is relevant only when future game launch support is implemented.
