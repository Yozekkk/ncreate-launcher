# NCreate Launcher

Официальный desktop-лаунчер NCreate, первый этап. Tauri v2, Rust, Vue 3.
Minimal, Standard и Ultra представлены в интерфейсе; установка и запуск игры
отключены до появления официальных manifests. Сторонние сборки не поддерживаются.

Это изменённая GPLv3-версия desktop-кода Modrinth App, а не приложение Modrinth.
Microsoft/Xbox/XSTS/Minecraft pipeline извлечён из актуальной main; серверная
часть сайта, marketplace и рекламная инфраструктура не включены.
Подробности и исходный commit: [NOTICE.md](NOTICE.md), [аудит](docs/UPSTREAM-AUDIT.md).
Лицензия: [GPL-3.0-only](LICENSE). ПО распространяется без гарантий.

## Разработка на EndeavourOS

Требуются Node.js 22.12+, pnpm 10.30.3, современный Rust и системные библиотеки:

```sh
sudo pacman -S --needed base-devel rust nodejs pnpm webkit2gtk-4.1 openssl librsvg patchelf libappindicator-gtk3 xdg-utils fuse2
cd /home/tima/ncreate-launcher
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
cargo fmt --all --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
pnpm app:build --bundles appimage,deb
```

Linux artifacts: `target/release/bundle/appimage/` и `target/release/bundle/deb/`.
Windows NSIS собирается workflow на Windows x64. [Инсталляторы и обновления](docs/RELEASE.md).
Updater отключён до настройки собственных signing keys. Modrinth updater не подключён.

`scripts/build.mjs` присваивает готовым пакетам имена
`NCreate-Launcher-0.1.0.AppImage`, `NCreate-Launcher-0.1.0-amd64.deb` и
`NCreate-Launcher-Setup-0.1.0.exe`. На Linux wrapper задаёт `NO_STRIP=true`:
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
[Политика сети](docs/NETWORK.md).

Offline UUID: Java-совместимый MD5 `OfflinePlayer:<nickname>` с UUID v3 variant;
регистр символов значим. Переименование меняет UUID и локальную identity.
Скины Offline доступны через публичный Ely.by Skin System, без пароля. Запросы
выполняет Rust; PNG проверяется и кешируется. Fallback — локальный Steve.
Skin providers: Mojang, Ely.by, Fallback. Полная Ely.by authentication и внедрение
скина в игру относятся к следующему этапу.

Edition definitions: `apps/app-frontend/src/models.ts`. Все manifests сейчас `null`.
Будущее подключение установки должно добавить проверенный backend-модуль и
доверенные manifests; одного изменения текста кнопки недостаточно.

## Проверка интерфейса

Development bridge для `tauri-agent-tools` собирается только с `debug_assertions`;
в production его модуль и IPC-команда отсутствуют. Локальные screenshots/logs
находятся в игнорируемом каталоге `verification/`; фактические результаты
фиксируются в [чеклисте](docs/REQUIREMENTS.md).

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
Linux packaging сейчас повторяется после исправления linuxdeploy; успешная
проверка Windows installer и удалённых GitHub workflows ещё не заявляется.
Для упаковки `patchelf` должен быть доступен в `PATH`, как и остальные
системные prerequisites. Баннер отсутствия сети проверен симуляцией состояния
`navigator`, без физического отключения сети.
