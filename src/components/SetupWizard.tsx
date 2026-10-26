// The first-run setup: five questions whose answers become the defaults of new sessions (lib/setup.ts).

import { openUrl } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  ArrowLeft,
  ArrowRight,
  BadgeCheck,
  CheckCircle2,
  Circle,
  Cpu,
  DollarSign,
  Eye,
  FileSpreadsheet,
  FolderOpen,
  Gauge,
  Layers,
  ScanLine,
  Search,
  Sparkles,
  Trophy,
  type LucideIcon,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { create } from "zustand";
import { api, errorText, type SettingsView, type SourceKind } from "../lib/api";
import { answersFromSettings, answersToSettings, type Goal, type Interest, type SetupAnswers } from "../lib/setup";
import { ModelPicker, useModels } from "./LocalModels";
import Logo from "./Logo";
import { ErrorNote } from "./Modal";
import { GAME_LABELS, TCG_GAMES } from "./NewSessionDialog";

/** The settings the showing setup starts from (none: hidden), and a count of finished setups, so pages reload. */
export const useSetupWizard = create<{
  settings: SettingsView | null;
  finished: number;
  show: (settings: SettingsView) => void;
  done: () => void;
}>((set) => ({
  settings: null,
  finished: 0,
  show: (settings) => set({ settings }),
  done: () => set((s) => ({ settings: null, finished: s.finished + 1 })),
}));

type Step = "interests" | "games" | "goals" | "sources" | "engine";

export default function SetupWizard({ settings }: { settings: SettingsView }) {
  const done = useSetupWizard((s) => s.done);
  // A first run starts blank; a rerun starts from the current defaults.
  const [answers, setAnswers] = useState<SetupAnswers>(() =>
    settings.setupDone
      ? answersFromSettings(settings)
      : { interests: [], games: [], goals: [], sources: [], engine: settings.engine },
  );
  const [step, setStep] = useState<Step>("interests");
  const [token, setToken] = useState("");
  const [localModel, setLocalModel] = useState(settings.localModel);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const models = useModels();

  const tcgOnly = answers.interests.length === 1 && answers.interests[0] === "tcg";
  const steps: Step[] = ["interests", ...(tcgOnly ? (["games"] as const) : []), "goals", "sources", "engine"];
  const index = steps.indexOf(step);
  const last = index === steps.length - 1;
  const missing =
    (step === "interests" && !answers.interests.length) || (step === "sources" && !answers.sources.length);

  const toggle = <K extends "interests" | "games" | "goals" | "sources">(key: K, value: SetupAnswers[K][number]) =>
    setAnswers((a) => {
      const list = a[key] as string[];
      return { ...a, [key]: list.includes(value) ? list.filter((v) => v !== value) : [...list, value] };
    });

  const save = async (next: SettingsView) => {
    setBusy(true);
    setError(null);
    try {
      if (token.trim()) await api.setToken(token.trim());
      await api.saveSettings(next);
      done();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };
  const finish = () => save(answersToSettings(answers, { ...settings, localModel }));
  const skip = () => save({ ...settings, setupDone: true });

  return (
    <div className="fixed inset-0 z-50 flex flex-col overflow-y-auto bg-slate-50 dark:bg-slate-950">
      <header className="flex items-center gap-3 px-8 py-5">
        <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-rail text-white">
          <Logo className="h-7 w-7" />
        </span>
        <div className="flex-1">
          <div className="font-semibold">Set up Cardloader</div>
          <div className="text-xs text-slate-500">
            Step {index + 1} of {steps.length} · you can change everything later in Settings
          </div>
        </div>
        <button className="btn btn-ghost" onClick={skip} disabled={busy}>
          Skip
        </button>
      </header>
      <div className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-8 pb-8">
        <div className="mb-8 flex gap-1.5" aria-hidden>
          {steps.map((s, i) => (
            <div
              key={s}
              className={clsx("h-1 flex-1 rounded-full", i <= index ? "bg-accent" : "bg-slate-200 dark:bg-slate-800")}
            />
          ))}
        </div>

        {step === "interests" && (
          <Question title="What will you scan?" text="Pick everything you collect.">
            <Tiles>
              {(
                [
                  ["tcg", "Trading card games", "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…", Layers],
                  ["sport", "Sports cards", "Baseball, basketball, football, hockey, soccer…", Trophy],
                ] as [Interest, string, string, LucideIcon][]
              ).map(([value, label, hint, icon]) => (
                <Tile
                  key={value}
                  icon={icon}
                  label={label}
                  hint={hint}
                  selected={answers.interests.includes(value)}
                  onClick={() => toggle("interests", value)}
                />
              ))}
            </Tiles>
          </Question>
        )}

        {step === "games" && (
          <Question
            title="Which games?"
            text="Pick one and every new session is set to it, so Ximilar matches only that game. Pick several, or none, to choose per session."
          >
            <div className="grid grid-cols-3 gap-2">
              {TCG_GAMES.map((game) => (
                <Tile
                  key={game}
                  label={GAME_LABELS[game] ?? game}
                  compact
                  selected={answers.games.includes(game)}
                  onClick={() => toggle("games", game)}
                />
              ))}
            </div>
          </Question>
        )}

        {step === "goals" && (
          <Question title="What do you want to find out?" text="Pick everything you need.">
            <Tiles>
              <Tile icon={Search} label="Identification" hint="Name, set and number of every card" selected locked />
              {(
                [
                  ["grading", "Grading", "A full grade: corners, edges, surface and centering", BadgeCheck],
                  ["condition", "Condition", "A quick condition, e.g. Near Mint, for listings", Gauge],
                  ["prices", "Prices", "Market prices of each card", DollarSign],
                ] as [Goal, string, string, LucideIcon][]
              ).map(([value, label, hint, icon]) => (
                <Tile
                  key={value}
                  icon={icon}
                  label={label}
                  hint={hint}
                  selected={answers.goals.includes(value)}
                  onClick={() => toggle("goals", value)}
                />
              ))}
            </Tiles>
          </Question>
        )}

        {step === "sources" && (
          <Question
            title="How will you bring in cards?"
            text="Pick all you use. The first of them opens with New session."
          >
            <Tiles>
              {(
                [
                  ["scanner", "Scanner", "A document scanner with a feeder, e.g. RICOH fi-8170", ScanLine],
                  ["folder", "Folder of images", "Photos or scans already on this computer", FolderOpen],
                  ["watch_folder", "Watch folder", "Cards are analysed as new images arrive in a folder", Eye],
                  ["csv", "CSV file", "A list of cards or image paths", FileSpreadsheet],
                ] as [SourceKind, string, string, LucideIcon][]
              ).map(([value, label, hint, icon]) => (
                <Tile
                  key={value}
                  icon={icon}
                  label={label}
                  hint={hint}
                  selected={answers.sources.includes(value)}
                  onClick={() => toggle("sources", value)}
                />
              ))}
            </Tiles>
          </Question>
        )}

        {step === "engine" && (
          <Question title="What should identify the cards?">
            <Tiles>
              <Tile
                icon={Sparkles}
                label="Ximilar API"
                hint="Precise identification, prices and grading. Needs an API token and internet."
                selected={answers.engine === "ximilar"}
                onClick={() => setAnswers((a) => ({ ...a, engine: "ximilar" }))}
              />
              <Tile
                icon={Cpu}
                label="Local AI model"
                hint="Runs on this computer, offline and free. Reads name, number and set; less precise."
                selected={answers.engine === "local"}
                onClick={() => setAnswers((a) => ({ ...a, engine: "local" }))}
              />
            </Tiles>
            {answers.engine === "ximilar" ? (
              <div className="mt-6">
                <label className="label">API token</label>
                <input
                  className="input"
                  type="password"
                  value={token}
                  placeholder={settings.tokenSet ? "Saved already, leave empty to keep it" : "Paste your token"}
                  onChange={(e) => setToken(e.target.value)}
                />
                <p className="mt-1.5 text-xs text-slate-500">
                  Find it in the{" "}
                  <button className="text-accent underline" onClick={() => openUrl("https://app.ximilar.com")}>
                    Ximilar App
                  </button>{" "}
                  under your profile. You can add it later in Settings.
                </p>
              </div>
            ) : (
              <div className="mt-6 space-y-3">
                {answers.goals.length > 0 && (
                  <p className="text-sm text-amber-700 dark:text-amber-400">
                    Grading, condition and prices need the Ximilar API; the local model only identifies cards.
                  </p>
                )}
                {models && <ModelPicker models={models} value={localModel} onChange={setLocalModel} />}
              </div>
            )}
          </Question>
        )}

        <div className="mt-auto pt-8">
          <ErrorNote>{error}</ErrorNote>
          <div className="mt-3 flex items-center justify-between">
            <button className="btn" onClick={() => setStep(steps[index - 1])} disabled={index === 0 || busy}>
              <ArrowLeft className="h-4 w-4" /> Back
            </button>
            {last ? (
              <button className="btn btn-primary" onClick={finish} disabled={busy}>
                Finish <CheckCircle2 className="h-4 w-4" />
              </button>
            ) : (
              <button className="btn btn-primary" onClick={() => setStep(steps[index + 1])} disabled={missing}>
                Next <ArrowRight className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Question({ title, text, children }: { title: string; text?: string; children: ReactNode }) {
  return (
    <section>
      <h1 className="text-2xl font-semibold">{title}</h1>
      {text && <p className="mt-1 text-slate-500">{text}</p>}
      <div className="mt-6">{children}</div>
    </section>
  );
}

function Tiles({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-2 gap-3">{children}</div>;
}

function Tile({
  icon: Icon,
  label,
  hint,
  selected,
  locked,
  compact,
  onClick,
}: {
  icon?: LucideIcon;
  label: string;
  hint?: string;
  selected: boolean;
  locked?: boolean;
  compact?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={locked}
      aria-pressed={selected}
      className={clsx(
        "flex items-start gap-3 rounded-lg border bg-white text-left transition-colors dark:bg-slate-900",
        compact ? "px-3 py-2" : "p-4",
        selected
          ? "border-accent ring-2 ring-accent/20"
          : "border-slate-200 hover:border-slate-300 dark:border-slate-800 dark:hover:border-slate-700",
        locked && "cursor-default opacity-80",
      )}
    >
      {Icon && (
        <span
          className={clsx(
            "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
            selected ? "bg-accent/10 text-accent" : "bg-slate-100 text-slate-500 dark:bg-slate-800",
          )}
        >
          <Icon className="h-5 w-5" />
        </span>
      )}
      <span className="min-w-0 flex-1">
        <span className={clsx("block font-medium", compact && "text-sm")}>{label}</span>
        {hint && <span className="mt-0.5 block text-sm text-slate-500">{hint}</span>}
        {locked && <span className="mt-0.5 block text-xs text-slate-400">Always included</span>}
      </span>
      {selected ? (
        <CheckCircle2 className="h-5 w-5 shrink-0 text-accent" />
      ) : (
        <Circle className="h-5 w-5 shrink-0 text-slate-300 dark:text-slate-600" />
      )}
    </button>
  );
}
