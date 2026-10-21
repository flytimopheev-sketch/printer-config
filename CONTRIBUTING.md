# Участие в проекте

Спасибо за интерес к Printer Config! Проект открыт для pull request'ов:
исправления ошибок, новые возможности, улучшение документации.

## Быстрый старт

```sh
git clone https://github.com/flytimopheev-sketch/printer-config.git
cd printer-config
npm install
npm run tauri:dev
```

Требования: Node.js 20.19+ (или 22.12+), Rust 1.90+, Windows 10/11 или
Linux с системными библиотеками Tauri (webkit2gtk4.1, gtk3, libayatana-appindicator3,
librsvg2, libxdo, libcups2).

## Проверки перед отправкой PR

```sh
# фронтенд: типы + сборка
npm ci
npm run build

# бэкенд: форматирование, линтер, тесты
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Те же проверки выполняет CI (`.github/workflows/rust.yml`) на Linux и Windows,
поэтому локальный прогон избавит от лишней переделки.

## Структура репозитория

```
.
├── src/                     # фронтенд: React + TypeScript
│   ├── App.tsx              # состояние приложения и навигация
│   ├── components/          # карточки, модальные окна, уведомления
│   ├── views/               # вкладки (принтеры, задания, обнаружение, о программе)
│   ├── lib/api.ts           # типизированные обёртки над Tauri invoke/event
│   └── styles.css           # оформление (тёмная тема)
├── src-tauri/               # бэкенд: Rust + Tauri
│   ├── src/commands.rs      # команды, доступные из UI
│   ├── src/winspool.rs      # Windows: Spooler API
│   ├── src/cups.rs          # Linux: CUPS
│   ├── src/discovery.rs     # mDNS-обнаружение
│   └── src/config.rs        # настройки приложения
├── packaging/Dockerfile.build   # контейнер для сборки RPM
├── reference/               # оригинал system-config-printer (справочно)
└── .github/                 # CI, шаблоны issue и pull request
```

## Как устроен код

* **Бэкенд выбирается на этапе компиляции.** В `src-tauri/src/lib.rs` модуль
  `backend` — это `winspool.rs` на Windows и `cups.rs` на Linux. Оба
  реализуют одинаковый набор функций, поэтому UI не знает о платформе.
* **UI ничего не знает о системных командах.** Всё общение идёт через
  Tauri-команды в `commands.rs`; их типы описаны в `src/lib/api.ts`.
* **Ошибки возвращаются строками на русском языке** и показываются пользователю
  как всплывающие уведомления.
* **Платформенные различия** учитываются через `#[cfg(windows)]` / `#[cfg(unix)]`
  и через команду `platform`, которая сообщает UI текущую ОС.

## Правила оформления

* Форматирование Rust — `cargo fmt`, TypeScript — 2 пробела, без точек с
  запятой в конце строк (как в текущем коде).
* Комментарии и сообщения об ошибках — на русском языке.
* Новые команды Tauri обязательно добавляются в `generate_handler!` и в
  типизированную обёртку `src/lib/api.ts`.
* Для Linux-команд, меняющих систему, учитывайте, что они могут выполняться
  от root (в CUPS-бэкенде используется `pkexec`).

## Как отправить изменения

1. Создайте ветку: `git checkout -b feature/short-description`.
2. Внесите изменения и проверьте код командами из раздела «Проверки».
3. Обновите `CHANGELOG.md` (раздел «Не выпущено»).
4. Откройте pull request в `main` — CI соберёт проект на Linux и Windows.

## Релизы

Релиз собирается автоматически по тегу `vX.Y.Z`:

```sh
# версия должна совпадать в package.json, src-tauri/Cargo.toml и tauri.conf.json
git tag v1.3.0
git push origin v1.3.0
```

CI соберёт RPM в контейнере Fedora 37, прогонит GUI smoke-тест под Xvfb и
опубликует `Printer-Config-<версия>-<релиз>.<arch>.rpm` в GitHub Releases.

## Лицензия

Новый код публикуется под лицензией MIT (см. [LICENSE](LICENSE)).
