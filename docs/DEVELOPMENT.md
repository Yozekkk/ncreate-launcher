# Development and architecture

Официальный desktop-лаунчер NCreate. Tauri v2, Rust, Vue 3.
Stage 2 добавляет отдельную библиотеку пользовательских instances и публичный
каталог Modrinth. Текущий `main` также подключает официальную сборку NCreate
Server 1.0.2 из отдельного [репозитория сборки](https://github.com/Yozekkk/ncreate-pack).

Это изменённая GPLv3-версия desktop-кода Modrinth App, а не приложение Modrinth.
Microsoft/Xbox/XSTS/Minecraft pipeline извлечён из актуальной main; серверная
часть сайта, marketplace и рекламная инфраструктура не включены.
Подробности и исходный commit: [NOTICE.md](../NOTICE.md), [аудит](UPSTREAM-AUDIT.md).
Лицензия: [GPL-3.0-only](../LICENSE). ПО распространяется без гарантий.

## Разработка на EndeavourOS

Требуются Node.js 22.12+, pnpm 10.30.3, современный Rust и системные библиотеки:

```sh
sudo pacman -S --needed base-devel rust nodejs pnpm webkit2gtk-4.1 openssl librsvg patchelf libappindicator-gtk3 xdg-utils fuse2
git clone https://github.com/Yozekkk/ncreate-launcher.git
cd ncreate-launcher
pnpm install --frozen-lockfile
pnpm app:dev
```

Microsoft-токены сохраняются в Secret Service (например, разблокированный KDE
Wallet/gnome-keyring). Пароли Microsoft не принимаются самим лаунчером. Если
системное хранилище недоступно, вход не сохраняет токены в открытые файлы.

## Проверки и production build

```sh
pnpm prepr:frontend:app
pnpm build
cargo fmt --package ncreate-launcher --package ncreate-app-lib --package ncreate-launcher-core --check
cargo check --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets
cargo clippy --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core --all-targets --no-deps -- -D warnings
cargo test --locked -p ncreate-launcher -p ncreate-app-lib -p ncreate-launcher-core
pnpm app:build --bundles appimage,deb
```

Linux artifacts: `target/release/bundle/appimage/` и `target/release/bundle/deb/`.
Windows NSIS собирается workflow на Windows x64. [Инсталляторы и обновления](RELEASE.md).
Подписанный Tauri updater настроен в текущем `main`, но установленная публичная
v0.5.0 была собрана раньше и требует ручного перехода на следующий подписанный
выпуск. Modrinth updater не подключён.

`scripts/build.mjs` присваивает готовым пакетам имена
`NCreate-Launcher-<version>.AppImage`, `NCreate-Launcher-<version>-amd64.deb` и
`NCreate-Launcher-Setup-<version>.exe`. На Linux wrapper задаёт `NO_STRIP=true`:
старый `strip` внутри linuxdeploy не поддерживает RELR-секции современных
библиотек Arch. Собственный Rust binary уже обрабатывается release-профилем.
Локальная сборка EndeavourOS использует текущий glibc этой системы и не
подтверждает совместимость со старым Ubuntu. Для распространяемых Linux
релизов CI настроен на Ubuntu 22.04; его результат проверяется отдельно.

## Данные и приватность

Linux: `~/.local/share/ncreate-launcher/launcher.db`, кеш скинов в `skins/`.
Windows: `%LOCALAPPDATA%\ncreate-launcher\`. Tauri window state использует каталог
`com.ncreate.launcher` соответствующей платформы. Настройки и аккаунты не читаются
из Modrinth App и не мигрируются. Microsoft-токены — исключительно OS keyring
с service `com.ncreate.launcher`. Телеметрия отсутствует.
[Политика сети](NETWORK.md).

Offline UUID: Java-совместимый MD5 `OfflinePlayer:<nickname>` с UUID v3 variant;
регистр символов значим. Переименование меняет UUID и локальную identity.
Скины Offline доступны через публичный Ely.by Skin System, без пароля. Запросы
выполняет Rust; PNG проверяется и кешируется. Fallback — локальный Steve.
Skin providers: Mojang, Ely.by, Fallback. Stage 2 отделяет AccountProvider от GameIdentity, SkinProvider и
CredentialReference. Ely.by authentication использует официальные endpoints;
токены остаются в native keyring, пароль не записывается на диск.

Официальная карточка NCreate Server находится в `apps/app-frontend/src/App.vue`.
Production manifest URL задаётся в `packages/launcher-core/src/models.rs`,
установка и обновление — в `packages/launcher-core/src/editions.rs`. Публикация
сборки отделена от лаунчера; её исходники и инструкция находятся в
[ncreate-pack](https://github.com/Yozekkk/ncreate-pack). См. [обновления](UPDATES.md)
и [Stage 2 architecture](STAGE2-ARCHITECTURE.md).

## Проверка интерфейса и история v0.1.0

Development bridge для `tauri-agent-tools` собирается только с `debug_assertions`;
в production его модуль и IPC-команда отсутствуют. Локальные screenshots/logs
находятся в игнорируемом каталоге `verification/`; фактические результаты
фиксируются в [чеклисте](REQUIREMENTS.md). Итоговые осмотренные экраны
сохранены также в [docs/screenshots](screenshots).

Проверены реальный Tauri desktop под X11, Home/Accounts/Offline/Microsoft
sign-in/Settings и размер окна 860×620. Два офлайн профиля сохранились после
перезапуска; переключение и смена UUID при переименовании проверены. Реальный
Ely.by skin и локальный fallback отобразились в интерфейсе. Microsoft flow
дошёл до официальной страницы входа; завершение требует интерактивной
авторизации владельца аккаунта. Сохранение Microsoft-профиля после полного
входа пока не подтверждено. Нативное хранилище секретов отдельно проверено
записью, чтением и удалением тестовой записи.

Перезапуск самостоятельного debug binary проверен с новым PID и сохранёнными
данными. При запуске через `pnpm app:dev` Tauri CLI завершает дочерний процесс
перезапуска; production-перезапуск проверяется отдельно на готовом artifact.
Linux AppImage и DEB собраны и проверены локально. Windows NSIS и Linux
installers успешно собраны GitHub Actions для commit
`22b5c7f8311d5b8b6d0d783af871caf8df016250` (run `36319792086`).
Установка и GUI Windows ещё не проверены на настоящей Windows-системе.
Для упаковки `patchelf` должен быть доступен в `PATH`, как и остальные
системные prerequisites. Баннер отсутствия сети проверен симуляцией состояния
`navigator`, без физического отключения сети.
