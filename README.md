# printer-config

Windows-клиент для управления принтерами и очередями печати (Tauri 2 + React 19 + Vite).

Поддерживаемые возможности:
- Список принтеров/очередей и установка принтера по умолчанию.
- Пакеты печати: перечисление, отмена, пауза/продолжение, перезапуск.
- mDNS/DNS-SD-сканнер сетевых принтеров (ZeroConf).
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
Шаблон репозитория уже содержит `dist/`, `README.md`, `packaging/rpm/` и CI-скилл. После создания репозитория GitHub выполните:

```ps
git remote add origin https://github.com/<user>/printer-config.git
git branch -M main
git push -u origin main
```

## Релиз под Red Hat Enterprise Linux / RedOS (RPM)
Пакет в `packaging/rpm/` упаковывает веб-интерфейс приложения и лаунчер. Бэкенд (Native Windows Spooler)
начал разрабатываться и не является кросс-платформенным — для работы под Linux требует замены на
`libcups`/`lp` (пока не выполнено; требование проекта оставляется как TODO).

### Сборка RPM на AlmaLinux / Rocky / RedOS
```sh
# один раз: установить инструменты
sudo dnf install -y rpm-build createrepo-c

# собрать .rpm
rpmbuild -bb packaging/rpm/printer-config.spec \
  --define "_srcrpmdir /tmp/rpmbuild/SRPMS" \
  --define "_rpmdir /tmp/rpmbuild/RPMS"
```

RPM-файл появится в `/tmp/rpmbuild/RPMS/noarch/printer-config-*.rpm` и можно установить:
```sh
sudo dnf install -y /tmp/rpmbuild/RPMS/noarch/printer-config-*.rpm
# или через веб-интерфейс: менеджер пакетов -> локальные файлы
```

Важно: `printer-config.spec` ссылается на `dist/` (от коммита), поэтому перед сборкой RPM убедитесь,
что `npm run build` выполнен поверх официального релиза.