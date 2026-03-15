import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { BrowserRouter } from "react-router-dom";
import "./index.css";
import "./i18n/index.ts";
import App from "./App.tsx";
import { AppPreferencesProvider } from "./contexts/appPreferencesProvider.tsx";
import { ThemeAndBaseline } from "./providers/ThemeAndBaseline.tsx";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <AppPreferencesProvider>
        <ThemeAndBaseline>
          <App />
        </ThemeAndBaseline>
      </AppPreferencesProvider>
    </BrowserRouter>
  </StrictMode>,
);
