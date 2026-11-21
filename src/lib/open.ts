import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { toastError } from "../components/Toast";

// Opening outside the app goes through Tauri's opener, which refuses what the capabilities don't allow: a refusal
// says why instead of the click doing nothing.

/** A web page in the default browser. */
export const openLink = (url: string) => openUrl(url).catch(toastError);
/** A file or folder in its default app. */
export const openFile = (path: string) => openPath(path).catch(toastError);
/** A file shown in its folder. */
export const reveal = (path: string) => revealItemInDir(path).catch(toastError);
