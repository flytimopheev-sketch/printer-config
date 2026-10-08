export default function AboutView() {
  return (
    <div className="view">
      <div className="view__header">
        <div>
          <h2 className="view__title">О программе</h2>
          <p className="view__sub">Краткая справка по приложению</p>
        </div>
      </div>

      <div className="about-card">
        <p className="about-card__lead">
          <b>Printer Config</b> — это локальное десктоп-приложение для управления
          печатными устройствами под управлением Windows.
        </p>

        <ul className="about-card__list">
          <li>
            <strong>Список принтеров:</strong> чтение и изменение параметров через
            Windows Spooler API (lpx.dll).
          </li>
          <li>
            <strong>Очередь заданий:</strong> просмотр, отмена, пауза и перезапуск
            заданий на выбранном принтере.
          </li>
          <li>
            <strong>Обнаружение:</strong> автономное сканирование сети по протоколу
            mDNS/DNS-SD для поиска сетевых и сетевых принтеров.
          </li>
          <li>
            <strong>Настройки:</strong> сохранение состояния приложения в
            %APPDATA%\printer-config\config.json.
          </li>
        </ul>

        <p className="about-card__meta">
          Версия: 0.1.0 · Платформа: Windows · UI: React 19 + Tauri 2 · Backend: Rust
        </p>
      </div>
    </div>
  );
}
