import clsx from "clsx";
import { LayoutDashboard, Layers, Settings, WalletCards } from "lucide-react";
import { NavLink, useLocation } from "react-router-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import ximilarLogo from "../assets/ximilar.svg";
import { isUnsaved, useUnsavedSettings } from "../pages/SettingsPage";
import Logo from "./Logo";

const NAV = [
  { to: "/", label: "Home", icon: LayoutDashboard, match: [] as string[] },
  { to: "/sessions", label: "Sessions", icon: Layers, match: ["/sessions", "/remote"] },
  { to: "/cards", label: "Cards", icon: WalletCards, match: ["/cards"] },
  { to: "/settings", label: "Settings", icon: Settings, match: ["/settings"] },
];

export default function Sidebar() {
  const { pathname } = useLocation();
  const unsaved = useUnsavedSettings((s) => isUnsaved(s.saved, s.draft));
  return (
    <nav className="flex w-[76px] shrink-0 flex-col items-center bg-rail py-3 text-white">
      <div className="mb-5 flex flex-col items-center" title="Cardloader">
        <Logo className="h-10 w-10" />
        <span className="mt-1 text-[10px] font-semibold tracking-wide text-white/80">cardloader</span>
      </div>
      <div className="flex flex-1 flex-col items-center gap-1">
        {NAV.map(({ to, label, icon: Icon, match }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) => {
              const active = isActive || match.some((m) => pathname.startsWith(m));
              return clsx(
                "flex w-16 flex-col items-center gap-1 rounded-lg py-2 text-[11px] font-medium transition-colors",
                active ? "bg-rail-active text-white" : "text-white/70 hover:bg-rail-hover hover:text-white",
              );
            }}
          >
            <span className="relative">
              <Icon className="h-5 w-5" strokeWidth={1.8} />
              {to === "/settings" && unsaved && (
                <span
                  className="absolute -top-0.5 -right-1 h-2 w-2 rounded-full bg-amber-400"
                  title="Unsaved settings"
                  aria-label="Unsaved settings"
                />
              )}
            </span>
            {label}
          </NavLink>
        ))}
      </div>
      <a
        href="https://www.ximilar.com"
        onClick={(e) => {
          e.preventDefault();
          void openUrl("https://www.ximilar.com");
        }}
        className="mt-3 px-2 opacity-70 transition-opacity hover:opacity-100"
        title="Powered by Ximilar"
      >
        <img src={ximilarLogo} alt="Ximilar" className="w-14" />
      </a>
    </nav>
  );
}
