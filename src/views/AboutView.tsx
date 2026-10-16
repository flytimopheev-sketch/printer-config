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
          <b>Printer Config</b> — локальное десктоп-приложение для управления
          сетевыми и локальными принтерами (CUPS/IPP, Windows Spooler).
        </p>

        <ul className="about-card__list">
          <li>
            <strong>Управление очередями CUPS:</strong> добавление, удаление,
            включение/выключение, настройка через lpadmin / lpoptions.
          </li>
          <li>
            <strong>Параметры принтера:</strong> состояние, уровень тонера/чернил,
            тестовая страница, опции PPD и опции заданий по умолчанию.
          </li>
          <li>
            <strong>Очередь заданий:</strong> просмотр, отмена, пауза и
            перезапуск заданий на выбранном принтере.
          </li>
          <li>
            <strong>Обнаружение:</strong> сканирование сети по mDNS/DNS-SD для
            поиска сетевых принтеров.
          </li>
        </ul>

        <p className="about-card__meta">
          Лицензия: MIT · Связь:
          <a href="mailto:flytimopheev@gmail.com" className="about-card__email">
            flytimopheev@gmail.com
          </a>
        </p>
      </div>
    </div>
  );
}