import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import { ConfigProvider } from "./lib/useConfig";
import { bootstrapServerConfig } from "./lib/server";
import "./styles/global.css";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// Splash while the embedded server boots (first run only; instant otherwise).
root.render(
  <div className="flex h-full w-full items-center justify-center bg-donna-bg text-sm text-gray-400">
    Starting Donna…
  </div>
);

bootstrapServerConfig().finally(() => {
  root.render(
    <React.StrictMode>
      <ConfigProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </ConfigProvider>
    </React.StrictMode>
  );
});
