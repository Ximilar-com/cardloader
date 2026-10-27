import { useEffect } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import SetupWizard, { useSetupWizard } from "./components/SetupWizard";
import Sidebar from "./components/Sidebar";
import Toaster from "./components/Toast";
import { api } from "./lib/api";
import CardsPage from "./pages/CardsPage";
import DashboardPage from "./pages/DashboardPage";
import RemoteSessionPage from "./pages/RemoteSessionPage";
import SessionPage from "./pages/SessionPage";
import SessionsPage from "./pages/SessionsPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  const { settings, finished, show } = useSetupWizard();

  // The first launch (and the first after an update that added the setup) starts with it.
  useEffect(() => {
    api.getSettings().then((s) => !s.setupDone && show(s));
  }, [show]);

  return (
    <div className="flex h-full overflow-hidden">
      <Sidebar />
      {/* Keyed: pages load their settings again after a setup. */}
      <main key={finished} className="min-w-0 flex-1 overflow-hidden">
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
