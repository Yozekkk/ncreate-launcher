# Network and privacy audit

Upstream commit: `0e817be41cebdb0e140866e87e727c2101fe0e6b`.
Audit covers desktop shell, frontend, application library, API client and
analytics source. The standalone extraction omits the upstream product
modules rather than retaining their network clients behind hidden navigation.

## Removed upstream destinations

| Destination | Upstream source | Purpose omitted from NCreate |
| --- | --- | --- |
| `https://api.modrinth.com` | `apps/app-frontend/src/config.ts:5` | Marketplace and Modrinth user services |
| `https://archon.modrinth.com` | `apps/app-frontend/src/config.ts:8` | Hosting |
| `https://shared-instances.modrinth.com` | `apps/app-frontend/src/config.ts:11` | Shared instances |
| `https://api.modrinth.com/appCriticalAnnouncement.json` | `apps/app-frontend/src/App.vue:891` | Product announcements |
| `https://api.modrinth.com/v2/surveys` | `apps/app-frontend/src/components/ui/SurveyPopup.vue:162` | Surveys |
| `https://modrinth.com/news/feed/articles.json` | `apps/app-frontend/src/App.vue:904` | News feed |
| `https://modrinth.com/wrapper/app-ads-cookie` | `apps/app/src/api/ads.rs:22` | Advertising session |
| `https://posthog.modrinth.com` | `apps/app-frontend/src/helpers/analytics.ts:68` | Analytics |
| `https://9508775ee5034536bc70433f5f531dd4@o485889.ingest.us.sentry.io/4504579615227904` | `apps/app-frontend/src/helpers/error-reporting.ts:23` | Remote error reporting |
| `https://launcher-files.modrinth.com/updates.json` | `apps/app/tauri-release.conf.json:34`, frontend `App.vue:2025` | Original product updater |
| `https://launcher-meta.modrinth.com` | `packages/api-client/src/modules/launcher-meta/v0.ts:6` | Loader manifest service |
| `https://cdn.modrinth.com` | `packages/app-lib/src/util/fetch.rs:972` | Marketplace downloads |
| `https://cdn.modrinth.com/fonts/minecraft/{regular,italic,bold,bold-italic}.otf` | frontend `assets/stylesheets/global.scss:12–36` | Remote fonts |
| `https://launcher-files.modrinth.com/assets/steve_head.png` | frontend `components/ui/AccountsCard.vue:26` | Old remote fallback avatar |
| `https://launcher-files.modrinth.com/assets/maze-bg.png` | frontend `components/ui/LegacyProjectCard.vue:71` | Old product artwork |
| `https://cdn.modrinth.com/modrinth-hosting-medal-{light,dark}.webp` | frontend `components/ui/PromotionWrapper.vue:42–47` | Hosting promotions |
| `https://cdn.modrinth.com/placeholder-banner.svg` | frontend `pages/project/Gallery.vue:67` | Marketplace artwork |
| `https://modrinth.host/medal` | frontend `components/ui/PromotionWrapper.vue:37` | Hosting promotion link |
| `https://modrinth.plus` | frontend `App.vue:1556` | Paid product link |
| `https://support.modrinth.com` | frontend error modals and shell `main.rs:415` | Product support links |
| `https://modrinth.com` project, organization, report, legal, billing, news and pride links | frontend project/instance/hosting pages | Product navigation |

Upstream `apps/app/tauri.conf.json:105–110` also admitted
`https://*.modrinth.com`, `https://*.nodes.modrinth.com`,
`wss://*.nodes.modrinth.com`, `https://*.posthog.com`, `https://*.sentry.io`,
`https://app.getsentry.com`, Stripe, Intercom, Tally and embedded video hosts.
Those product permissions and integrations are excluded. The executable
Minecraft auth module changes the upstream HTTP User-Agent string that
mentioned `https://modrinth.com/app`; a User-Agent is not an API request.
`packages/app-lib/src/upstream/minecraft_auth.rs` is a provenance copy,
not declared as a Rust module or executed; its original upstream User-Agent
remains as a source comparison record only.
Upstream GitHub source links remain only as attribution/documentation.

## Allowed functional requests

| Destination | Use |
| --- | --- |
| `https://login.live.com/oauth20_desktop.srf` | Microsoft OAuth redirect page |
| `https://login.live.com/oauth20_token.srf` | OAuth code/refresh exchange |
| `https://device.auth.xboxlive.com/device/authenticate` | Xbox device authentication |
| `https://sisu.xboxlive.com/authenticate`, `/authorize` | Xbox SISU authentication |
| `https://xsts.auth.xboxlive.com/xsts/authorize` | XSTS token |
| `https://api.minecraftservices.com/launcher/login` | Minecraft token |
| `https://api.minecraftservices.com/minecraft/profile` | UUID, nickname and skin metadata |
| `https://api.minecraftservices.com/entitlements/license` | Minecraft ownership |
| `https://textures.minecraft.net/texture/…` | Mojang skin image |
| `https://skinsystem.ely.by/skins/{nickname}.png` | Offline nickname skin lookup |
| `https://ely.by/storage/skins/…` | Ely.by skin image after its public lookup redirect |
| Microsoft sign-in WebView | OAuth browser page with exact `https://login.live.com/oauth20_desktop.srf` callback validation |

The `http://auth.xboxlive.com` and `http://xboxlive.com` values in the auth
protocol are relying-party/audience identifiers, not unencrypted HTTP token
destinations. No password is requested for Ely.by skins. Official references:
[Ely.by skin system](https://docs.ely.by/en/skins-system.html) and
[Ely.by launcher authentication](https://docs.ely.by/en/minecraft-auth.html).

Skin images are fetched by Rust and passed to the frontend as data URLs, so
the WebView does not need broad remote image permissions. Network failures
must yield a local avatar fallback and a usable offline profile.

The live Ely.by lookup for `ErickSkrauch` returned a `301` Location pointing to
`http://ely.by/storage/skins/…`. The Rust client disables automatic redirects
and follows them manually, upgrading this legacy HTTP location to HTTPS before
requesting the image. Every request permits only HTTPS port 443, no URL
credentials, and these exact host/path combinations: `skinsystem.ely.by/skins/`,
`ely.by/storage/skins/`, `textures.minecraft.net/texture/`. The loop permits at
most four request hops and rejects other destinations; it never downloads a
skin over HTTP. Downloads have a 1 MiB bound and decoded PNG limits of 64×64
(also permitting legacy 64×32 skins).

Source regression tests in `apps/app/src/skins.rs` check local fallback preview
(`fallback_is_valid_preview`) and malformed-image rejection
(`invalid_skin_is_rejected`). The 2026-09-26 test log recorded both passing,
along with offline persistence/single-active-account and deterministic UUID
tests. The final run also passed `skin_redirects_stay_on_public_texture_hosts`.
Runtime
verification additionally checks an
actual Ely.by profile lookup and the avatar rendered inside the desktop WebView.
That observed successful lookup does not prove all possible names or service
outage behavior; missing/network cases return local fallback or cached images.

## Privacy boundaries

No PostHog, Sentry, remote crash reporting, Modrinth account login, updater,
advertising or product startup requests are part of stage one. Account metadata
and launcher settings are local. Microsoft tokens are kept outside frontend
state in the operating-system credential store; access and refresh tokens must
never enter logs. Developer bridge access is development only.

## Reproduce source audit

From repository root:

```sh
rg -n -i 'modrinth\.(com|host|plus)|posthog|sentry|theseus' apps packages --glob '!**/LICENSE' --glob '!**/COPYING.md' --glob '!**/node_modules/**' --glob '!**/dist/**'
rg -n 'https?://' apps/app/src packages/app-lib/src apps/app/tauri.conf.json
```

Treat attribution and protocol audience strings separately from executable
network destinations. Runtime proof additionally requires desktop diagnostics;
a clean source scan is not a packet capture. Record any remaining production
destination and its reason before declaring this audit complete.
