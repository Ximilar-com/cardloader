import { create } from "zustand";
import { api, errorText, onUpdateProgress, type UpdateInfo, type UpdateProgress } from "./api";

type Status = "idle" | "checking" | "current" | "available" | "installing";

/** Updates from the app's GitHub releases: checked once at launch and from Settings › About. */
export const useUpdate = create<{
  status: Status;
  info: UpdateInfo | null;
  progress: UpdateProgress | null;
  error: string | null;
  /** A quiet check (at launch) shows no error: being offline isn't worth a message. */
  check: (quiet?: boolean) => Promise<void>;
  install: () => Promise<void>;
}>((set, get) => ({
  status: "idle",
  info: null,
  progress: null,
  error: null,
  check: async (quiet = false) => {
    if (get().status === "checking" || get().status === "installing") return;
    set({ status: "checking", error: null });
    try {
      const info = await api.checkUpdate();
      set({ status: info ? "available" : "current", info });
    } catch (e) {
      set({ status: "idle", error: quiet ? null : errorText(e) });
    }
  },
  install: async () => {
    if (get().status !== "available") return;
    set({ status: "installing", progress: null, error: null });
    const unlisten = await onUpdateProgress((progress) => set({ progress }));
    try {
      // The app restarts once it is installed, so this returns only on an error.
      await api.installUpdate();
    } catch (e) {
      set({ status: "available", error: errorText(e) });
    } finally {
      unlisten();
    }
  },
}));
