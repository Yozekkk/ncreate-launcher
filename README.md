<p align="center">
  <strong>Русский</strong> •
  <a href="README.en.md">English</a>
</p>

<div align="center">
  <img src="apps/app-frontend/public/brand/logo.webp" alt="Логотип NCreate" width="112" height="112">
  <h1>NCreate Launcher v0.5.0 Beta</h1>
  <p>Десктопный Minecraft-лаунчер проекта NCreate для Windows и Linux.</p>
  <p>
    <img src="https://img.shields.io/badge/%D0%B2%D0%B5%D1%80%D1%81%D0%B8%D1%8F-v0.5.0%20Beta-f59e0b?style=flat-square" alt="Версия v0.5.0 Beta">
    <a href="https://github.com/Yozekkk/ncreate-launcher/releases/latest"><img src="https://img.shields.io/github/v/release/Yozekkk/ncreate-launcher?style=flat-square&color=f59e0b&label=%D1%80%D0%B5%D0%BB%D0%B8%D0%B7" alt="Последний опубликованный релиз"></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-555?style=flat-square" alt="Windows 10 и 11 x64">
    <img src="https://img.shields.io/badge/Linux-x64-555?style=flat-square&logo=linux&logoColor=white" alt="Linux x64">
    <a href="https://github.com/Yozekkk/ncreate-launcher/actions/workflows/check.yml"><img src="https://img.shields.io/github/actions/workflow/status/Yozekkk/ncreate-launcher/check.yml?branch=main&style=flat-square&label=CI" alt="Проверки CI"></a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Tauri-v2-555?style=flat-square&logo=tauri&logoColor=f59e0b" alt="Tauri v2">
    <img src="https://img.shields.io/badge/Rust-555?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Vue-3-555?style=flat-square&logo=vuedotjs&logoColor=white" alt="Vue 3">
    <a href="LICENSE"><img src="https://img.shields.io/badge/%D0%BB%D0%B8%D1%86%D0%B5%D0%BD%D0%B7%D0%B8%D1%8F-GPLv3-555?style=flat-square" alt="Лицензия GPLv3"></a>
  </p>
</div>

## Скачать

> [!IMPORTANT]
> [!NOTE]
> **v0.5.0 Beta** — функциональный лаунчер с Библиотекой, установкой Minecraft и каталогом модов. Официальные сборки NCreate Minimal, Standard и Ultra пока недоступны; другие ограничения перечислены ниже. Установщики появятся на странице релизов после проверки Windows и Linux сборок.

| Платформа                                                                | Файл v0.5.0 Beta                   | Скачать                                                                |
| :----------------------------------------------------------------------- | :--------------------------------- | :--------------------------------------------------------------------- |
| **Windows 10/11 x64**                                                    | `NCreate-Launcher-Setup-0.5.0.exe` | [Открыть релизы](https://github.com/Yozekkk/ncreate-launcher/releases) |
| **Linux x64** — EndeavourOS, Arch, Manjaro, Fedora и другие дистрибутивы | `NCreate-Launcher-0.5.0.AppImage`  | [Открыть релизы](https://github.com/Yozekkk/ncreate-launcher/releases) |
| **Debian, Ubuntu, Linux Mint** и совместимые системы                     | `NCreate-Launcher-0.5.0-amd64.deb` | [Открыть релизы](https://github.com/Yozekkk/ncreate-launcher/releases) |

[Все релизы](https://github.com/Yozekkk/ncreate-launcher/releases) · [Последний релиз](https://github.com/Yozekkk/ncreate-launcher/releases/latest)

## Что нового в v0.5.0

По сравнению с v0.1.0 лаунчер получил полноценную Библиотеку Minecraft и работу с публичным каталогом модов. Ниже перечислены проверенные возможности и границы их проверки.

- **Библиотека:** отдельные экземпляры Minecraft, создание, переименование, дублирование, импорт и экспорт `.mrpack`.
- **Игра:** установка и запуск Vanilla и Fabric проверены в Linux. Установка Forge и запуск его процесса также проверены; для Quilt и NeoForge остаются ограничения проверки, указанные ниже.
- **Моды и сборки:** публичный каталог Modrinth, поиск, совместимая установка модов и сборок, включение, отключение, удаление, обновление и откат модов.
- **Аккаунты:** локальные профили, протоколы входа Microsoft и Ely.by, публичные скины Ely.by.
- **Подготовка NCreate:** отдельная система манифестов, проверка файлов, управление загрузками и восстановление при неудачном обновлении. Официальные Minimal, Standard и Ultra пока недоступны.

Проверки отдельных функций описаны в [отчёте Stage 2](docs/STAGE2-VERIFICATION.md). Успешный вход с реальным аккаунтом Microsoft или Ely.by требует отдельной проверки владельцем.

## Скриншоты

Снимки сделаны в настоящем Linux-приложении из текущей ветки `main`.

<p align="center">
  <img src="docs/screenshots/stage2/home.png" alt="Главная страница NCreate Launcher с Minimal, Standard и Ultra" width="900">
</p>

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/stage2/library.png" alt="Библиотека Minecraft"></td>
    <td width="50%"><img src="docs/screenshots/stage2/content-mods.png" alt="Каталог модов"></td>
  </tr>
  <tr><td align="center">Библиотека</td><td align="center">Моды и сборки</td></tr>
  <tr>
    <td><img src="docs/screenshots/stage2/accounts.png" alt="Страница аккаунтов"></td>
    <td><img src="docs/screenshots/stage2/settings.png" alt="Настройки лаунчера"></td>
  </tr>
  <tr><td align="center">Аккаунты</td><td align="center">Настройки</td></tr>
</table>

## Возможности

В v0.5.0 доступны отдельные экземпляры Minecraft, поиск модов и сборок, локальные аккаунты и настройки. Библиотека и каталог используют интерфейс NCreate; реклама и вход в аккаунт Modrinth не требуются.

## Библиотека

Можно создавать отдельные экземпляры Vanilla, Fabric, Forge и Quilt, менять их название и настройки, дублировать, удалять, импортировать и экспортировать `.mrpack`. Для каждого экземпляра доступны свои моды, объём памяти и путь к Java. Выбор NeoForge также есть, но сквозной запуск конкретной версии ещё не подтверждён: список совместимых версий зависит от доступных метаданных загрузчика.

## Моды и сборки

Лаунчер использует публичный каталог Modrinth для поиска модов и сборок. Можно установить совместимую версию в выбранный экземпляр, управлять установленными модами и импортировать `.mrpack` как новый экземпляр. Проверены обновление и откат отдельного мода; автоматическое обновление **версии целой сборки** пока не реализовано.

Modrinth здесь — источник контента. **NCreate Launcher не является официальным приложением Modrinth** и не требует аккаунт Modrinth для публичного каталога.

## Аккаунты

| Тип           | Поддержка в v0.5.0 Beta                                                                                                                                                                |
| :------------ | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Offline**   | Локальный профиль с никнеймом и постоянным детерминированным UUID; создание, переключение и сохранение после перезапуска проверены.                                                    |
| **Microsoft** | Официальная цепочка входа OAuth/Minecraft реализована; технические проверки и полный синтетический сценарий прошли. Успешный вход с реальным аккаунтом владельца ещё не подтверждён.   |
| **Ely.by**    | Реализованы протокол входа и работа со скинами. Получение публичного скина и резервный вариант проверены с живым сервисом; успешный вход в реальный Ely.by аккаунт ещё не подтверждён. |

Offline и Ely.by профили не дают доступ к серверам, требующим лицензионный Microsoft-аккаунт. Подробные границы проверки — в [отчёте Stage 2](docs/STAGE2-VERIFICATION.md).

## Официальные версии NCreate

Minimal, Standard и Ultra остаются отдельными версиями NCreate. Их карточки видны на главной странице, но кнопки установки отключены: **официальные сборки NCreate пока готовятся**. Система манифестов и обновлений проверена на тестовых данных; манифесты для выпуска не опубликованы.

Автоматическое обновление самого лаунчера пока отключено: для него нужны подписанные релизы NCreate.

## Установка

Выберите файл v0.5.0 для своей системы на странице релиза и установите его по инструкции ниже.

### Windows 10/11 x64

Скачайте `.exe`, запустите установщик и откройте NCreate Launcher. Установщик beta-версии пока не подписан коммерческим сертификатом, поэтому Windows SmartScreen может показать предупреждение. Проверьте источник и контрольную сумму файла; не отключайте защиту Windows. Windows CI собирает NSIS, но установка и GUI на физическом Windows-компьютере ещё не проверены.

### Linux AppImage

AppImage — рекомендуемый переносимый вариант для EndeavourOS, Arch Linux, Manjaro, Fedora и других современных дистрибутивов. После загрузки файла запустите:

```bash
chmod +x NCreate-Launcher-0.5.0.AppImage
./NCreate-Launcher-0.5.0.AppImage
```

Если файл находится в `~/Загрузки`, сначала перейдите туда командой `cd ~/Загрузки`.

Если в системе нет FUSE 2, можно использовать `APPIMAGE_EXTRACT_AND_RUN=1 ./NCreate-Launcher-0.5.0.AppImage`.

### Debian, Ubuntu и Linux Mint

DEB предназначен для Debian/Ubuntu-совместимых систем. После загрузки файла установите его командой:

```bash
cd ~/Загрузки
sudo apt install ./NCreate-Launcher-0.5.0-amd64.deb
```

Для EndeavourOS и Arch выбирайте AppImage, а не DEB.

### Проверка загруженного файла

Скачайте `SHA256SUMS.txt` из того же релиза и сравните хеш файла со строкой в списке. Например, для AppImage v0.5.0 в Linux:

```bash
sha256sum NCreate-Launcher-0.5.0.AppImage
```

В Windows для установщика v0.5.0:

```powershell
Get-FileHash .\NCreate-Launcher-Setup-0.5.0.exe -Algorithm SHA256
```

## Первый запуск

После установки:

1. Откройте NCreate Launcher и добавьте аккаунт в разделе **Аккаунты**.
2. В **Библиотеке** создайте экземпляр Minecraft или импортируйте `.mrpack`.
3. При необходимости найдите совместимые моды в разделе **Моды и сборки**.
4. Установите Minecraft для выбранного экземпляра и нажмите **Играть**.

## Системные требования

- Windows 10/11 x64 или Linux x64; работа GUI непосредственно на Windows ещё требует проверки.
- Свободное место для Minecraft и выбранных модов; объём зависит от версии и содержимого.
- Подключение к интернету для первой загрузки игры, загрузчиков и контента.
- Java нужной для выбранной версии Minecraft версии. Лаунчер проверяет установленную Java и допускает собственный путь; автоматическая загрузка Java пока не реализована.

## Приватность

В приложении нет рекламы и отправки телеметрии в Modrinth, PostHog или Sentry. Данные аккаунтов и настроек остаются в отдельном каталоге NCreate; данные оригинального Modrinth App не импортируются. Токены Microsoft и Ely.by хранятся в системном хранилище учётных данных и не передаются в интерфейс. Пароли не сохраняются: пароль Ely.by, если он нужен для входа, используется только во время запроса. Подробнее — в [сетевом аудите](docs/NETWORK.md).

## Разработка

Нужны Node.js 22.12+, pnpm 10.30.3, стабильный Rust и [зависимости Tauri для Linux](docs/DEVELOPMENT.md).

```bash
git clone https://github.com/Yozekkk/ncreate-launcher.git
cd ncreate-launcher
pnpm install --frozen-lockfile
pnpm app:dev
```

Сборка Linux AppImage и DEB:

```bash
pnpm app:build --bundles appimage,deb
```

На Windows используйте `pnpm app:build --bundles nsis`. Подробности — в [руководстве разработчика](docs/DEVELOPMENT.md).

## Документация

- [Разработка и сборка](docs/DEVELOPMENT.md)
- [Сеть и приватность](docs/NETWORK.md)
- [Архитектура Stage 2](docs/STAGE2-ARCHITECTURE.md)
- [Фактические проверки Stage 2](docs/STAGE2-VERIFICATION.md)
- [Требования и ограничения](docs/REQUIREMENTS.md)
- [Публикация релизов](docs/RELEASE.md)
- [Происхождение исходного кода](docs/UPSTREAM-AUDIT.md)
- [Заметки выпуска v0.5.0](docs/releases/v0.5.0.md)

## Лицензия и авторство

NCreate Launcher использует изменённые части открытого кода [Modrinth App](https://github.com/modrinth/code) на условиях [GPLv3](LICENSE). Исходные уведомления и сведения об авторстве сохранены в [NOTICE.md](NOTICE.md), [COPYING.md](COPYING.md) и [аудите upstream](docs/UPSTREAM-AUDIT.md). Логотип и оформление принадлежат NCreate. Проект не связан с Mojang или Microsoft.
