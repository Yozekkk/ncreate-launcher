# 1.0.0 implementation audit

Baseline: `d10d7ea2538b815541ad9decbe97c329c2aa3541`. Only the user-owned
`обновление осени/` directory was untracked. Root AGENTS.md read before work.
Reference cloned outside the product into `/tmp/ncreate-onelauncher-reference`,
commit `7a6a7cbc0bbc9cf7a27e74fb700b4cbec4c995b9`.

## Visual mapping

| Reference source | Vue adaptation |
| --- | --- |
| theme/colors.rs | page #11171c, elevated #151c22, control #1a2228, foreground #d5dbff, secondary #78818d, white 5% borders; NCreate orange replaces blue brand |
| theme/mod.rs, navbar/app_navbar.rs | 80px top navigation, 40px sides, 36px links, compact branding below 1100px |
| layout/app_shell.rs, home/index.rs, active_cluster_panel.rs | full-page art with dark vignette, 48px home inset, prominent selected official instance and action |
| layout/settings_shell.rs | 225px contextual sidebar, 33px rows, grouped settings |
| button.rs | 6/8/10px radii; 12/14/18px text; 32px icon control; distinct hover/pressed/disabled |
| text_input.rs, dropdown.rs | 8px radius, 8×12px inset, outlined dark controls |
| toggle.rs, segmented_control.rs | 40×22px switch, 16px handle, 180ms transition; 9px segmented container |
| overlay_popup.rs, app_shell.rs | 560px error modal, 16px radius, 24px inset |
| progress_track.rs, spinner.rs, toasts.rs | rounded track, 900ms spinner, compact elevated notifications |
| clusters/page.rs, version_card.rs | compact library cards, 150px art, 24px column gaps |
| browser/index, settings/accounts.rs, account_switcher.rs | catalogue cards/filters, account surfaces, 300px account menu |

NCreate Vue handlers, authentication, IPC, manifests, updates, storage and launch
engine remain in place. No reference runtime or product services are imported.
Poppins has no Cyrillic coverage; existing local Manrope remains the Cyrillic font.

## Functional audit

Existing pages: Home official pack install/resume/update/launch; Library CRUD,
loader versions, mods, import/export, rollback, stop; Content Modrinth browse,
compatible versions and installation; Accounts offline/Microsoft/Ely.by and skins;
Settings RAM, Java path, appearance, autostart, channel/updater/restart/About.
Root AGENTS.md's initial-stage manifest/updater note is outdated: production
NCreate GitHub manifests and signed-updater code are present in the baseline.

Java probes execute -XshowSettings:properties -version with a 10s timeout and
kill-on-drop, but discovery is sequential and has no total limit. Architecture
falls back silently to amd64. No download/provisioning exists. Exact Java major
selection already exists; install and launch use merged metadata and stored
manifest overrides. Windows discovery omits local managed root. Installation
persists an automatically chosen path as though it were a manual override.

## Seasonal inspection

All 16 JPG originals inspected individually, not just by filename/contact sheet.

| Filename suffix | Contents / decision |
| --- | --- |
| 04_21-19-06 | Autumn Minecraft scene; suitable full home background, preserve composition |
| 05_15-41-57 (2) | NCreate forest banner; alternative official card artwork |
| 05_15-41-57 | NCreate three-season banner; too bright for primary home composition |
| 05_15-41-58 (2) | Halloween ship illustration, lower-right signature; omit |
| 05_15-41-58 | Action illustration with edge text; omit |
| 05_15-41-59 (2) | Chest/candlestick sheet, label; omit |
| 05_15-41-59 (3) | Halloween object sheet, busy background; omit |
| 05_15-41-59 | Three pumpkins, bottom signature; alternative, omit |
| 05_15-42-00 (2) | Halloween objects sheet; omit |
| 05_15-42-00 (3) | Halloween objects, frame and labels; omit |
| 05_15-42-00 | Three pumpkins on purple; isolate middle pumpkin, no text |
| 05_15-42-01 (2) | Blue adventurer on black; omit (nonseasonal) |
| 05_15-42-01 | Campfire characters, green fringe; omit |
| 05_15-42-02 | Pig character with striped background corruption; omit |
| 05_15-42-03 | Purple character on black; omit |
| 05_15-42-04 | Tree creature with strong green fringe; omit |

Originals are never modified. No network seasonal imagery or generated imagery.
