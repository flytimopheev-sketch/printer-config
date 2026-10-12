# printer-config

Windows-клиент для управления принтерами и очередями печати (Tauri 2 + React 19 + Vite).

Поддерживаемые возможности:
- Список принтеров/очередей и установка принтера по умолчанию.
- Пакеты печати: перечисление, отмена, пауза/продолжение, перезапуск.
- mDNS/DNS-SD-сканнер сетевых принтеров (ZeroConf).
- Добавление сетевого принтера по IP: `192.168.1.50` (приложение само пробует
  IPP-пути `/ipp/print`, `/ipp` и raw-порт 9100), `192.168.1.50/Очередь`,
  UNC-путь или полный URI (`ipp://…`, `socket://…`).
- Хранение конфигурации в `%APPDATA%\printer-config\config.json`.
- Тёмная тема, всплывающие уведомления (toast), модальное окно добавления сетевого принтера.

## Стек
- **Фронтенд**: React 19, TypeScript, Vite 7, Tauri 2-Web-разработка (UI).
- **Бэкенд**: Rust 1.98, Tauri 2, `winspool` (Windows Spooler API) + mDNS (`mdns_ssdp`/настраиваемый сканер).

## Требования (Windows)
- Windows 10/12 x64
- Node.js 20+ + npm
- Rust toolchain (выполнен `rustup`)
- PowerShell 7 + .NET (Tauri v2)

## Разработка (Vite)
```ps
npm install
npm run dev
```

## Сборка
```ps
# фронтенд + бэкенд (Tauri)
npm run build

# или по отдельности
npm run build:frontend
npm run build:tauri
```

## Запуск собранного
```ps
npm run preview
```

## GitHub и выпуск
Шаблон репозитория уже содержит `dist/`, `README.md` и CI. После создания репозитория GitHub выполните:

```ps
git remote add origin https://github.com/<user>/printer-config.git
git branch -M main
git push -u origin main
```

## Релиз под Red Hat Enterprise Linux / RedOS (RPM)
Бэкенд кроссплатформенный: под Linux он работает через **CUPS** (`libcups` — команда `lp`,
`lpadmin`, `lpinfo`, `cupsd`) и полностью покрывает очереди, задания, обнаружение и
конфигурацию (`src-tauri/src/cups.rs`); под Windows — через Spooler API.

RPM собирается **нативно через Tauri** (`tauri build`, цель `rpm` в `tauri.conf.json`) —
это один и тот же GUI-бинарник, что и под Windows, а не веб-обёртка. Сборка выполняется
в контейнере `packaging/Dockerfile.build` (Fedora 37, glibc 2.36 = РЕД ОС 8.0, поэтому
бинарник запускается без ошибки «GLIBC_x.xx not found»).

### Автоматический релиз (GitHub Actions)
Каждый тег `v*` запускает job `rpm` в `.github/workflows/rust.yml`:
1. собирает контейнер из `packaging/Dockerfile.build`;
2. выполняет `npm ci && npm run tauri build` внутри контейнера;
3. прогоняет GUI smoke-тест под Xvfb (окно «Printer Config» обязано появиться);
4. публикует `Printer-Config-<version>-<release>.<arch>.rpm` ассетом GitHub Release.

```sh
git tag v1.0.0
git push origin v1.0.0
```

### Локальная сборка RPM
```sh
# один раз: собрать образ
docker build -f packaging/Dockerfile.build -t printer-build .

# собрать .rpm (результат: src-tauri/target/release/bundle/rpm/*.rpm)
docker run --rm -v "$PWD":/app -w /app printer-build \
  bash -c 'npm ci && npm run tauri build'
```

Установка на РЕД ОС / AlmaLinux:
```sh
sudo dnf install -y "Printer-Config-"*.rpm
# или через менеджер пакетов -> локальные файлы
```

Зависимости RPM: `cups`, `cups-client`, `webkit2gtk4.1`, `gsettings-desktop-schemas`,
`libnotify`, `xdg-utils`, `desktop-file-utils` (список задан в `src-tauri/tauri.conf.json`).

### Заметка про GLib-GIO-CRITICAL
Сообщение вида `GLib-GIO-CRITICAL **: ... g_settings_schema_source_lookup: assertion 'source != NULL' failed`
при запуске GTK/WebKit-приложений означает, что в системе не установлены GSettings-схемы
 GNOME. Пакет `gsettings-desktop-schemas` в зависимостях RPM решает эту проблему; если
ошибка сохраняется — запустите приложение из терминала и пришлите полный текст строки
(после таймстемпа идёт имя функции, оно и указывает причину).