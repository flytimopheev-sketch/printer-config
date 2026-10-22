import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Убираем заставку из index.html: React рендерит интерфейс в тот же
// контейнер, но его содержимое нужно очистить явно.
const container = document.getElementById("root")!;
container.replaceChildren();

ReactDOM.createRoot(container).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
