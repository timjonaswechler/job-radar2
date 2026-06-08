import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "@/app/app";
import { AppProviders } from "@/app/app-providers";
import "@/styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </React.StrictMode>,
);
