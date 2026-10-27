import { useEffect } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import SetupWizard, { useSetupWizard } from "./components/SetupWizard";
import Sidebar from "./components/Sidebar";
import Toaster from "./components/Toast";
import { useI18n } from "./i18n";
import { api } from "./lib/api";
import { useUpdate } from "./lib/update";
import CardsPage from "./pages/CardsPage";
import DashboardPage from "./pages/DashboardPage";
import RemoteSessionPage from "./pages/RemoteSessionPage";
import SessionPage from "./pages/SessionPage";
import SessionsPage from "./pages/SessionsPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  const { settings, finished, show } = useSetupWizard();
  const language = useI18n((s) => s.language);

  // The first launch (and the first after an update that added the setup) starts with it.
  useEffect(() => {
    api.getSettings().then((s) => !s.setupDone && show(s));
  }, [show]);

  // Dev builds aren't installed bundles, so there is nothing to update; Settings can still check by hand.
  useEffect(() => {
    if (!import.meta.env.DEV) void useUpdate.getState().check(true);
  }, []);

  return (
    <div className="flex h-full overflow-hidden">
      <Sidebar />
      {/* Keyed: pages load their settings again after a setup, and build their texts again in a new language. */}
      <main key={`${finished}-${language}`} className="min-w-0 flex-1 overflow-hidden">
        <Routes>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/sessions" element={<SessionsPage />} />
          <Route path="/sessions/:id" element={<SessionPage />} />
          <Route path="/remote/:id" element={<RemoteSessionPage />} />
          <Route path="/cards" element={<CardsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
      {settings && <SetupWizard settings={settings} />}
      <Toaster />
    </div>
  );
}
