import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./app/App";
import { QueryProvider } from "./lib/query";
import "./styles/global.css";
const rootElement = document.getElementById("root");

if (rootElement === null) {
  throw new Error("Root element was not found.");
}

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <QueryProvider>
      <App />
    </QueryProvider>
  </React.StrictMode>,
);
