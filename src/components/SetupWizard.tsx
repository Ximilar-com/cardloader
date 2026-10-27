// The first-run setup: the app's language, then five questions whose answers become the defaults of new sessions
// (lib/setup.ts).

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
import { rich, useI18n, useT } from "../i18n";
import { LANGUAGES } from "../i18n/languages";
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

type Step = "language" | "interests" | "games" | "goals" | "sources" | "engine";

export default function SetupWizard({ settings }: { settings: SettingsView }) {
  const done = useSetupWizard((s) => s.done);
  const t = useT();
  const { language, setLanguage } = useI18n();
  // A first run starts blank; a rerun starts from the current defaults.
  const [answers, setAnswers] = useState<SetupAnswers>(() =>
    settings.setupDone
      ? answersFromSettings(settings)
      : { interests: [], games: [], goals: [], sources: [], engine: settings.engine },
  );
  const [step, setStep] = useState<Step>("language");
  const [token, setToken] = useState("");
  const [localModel, setLocalModel] = useState(settings.localModel);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const models = useModels();

  const tcgOnly = answers.interests.length === 1 && answers.interests[0] === "tcg";
  const steps: Step[] = [
    "language",
    "interests",
    ...(tcgOnly ? (["games"] as const) : []),
    "goals",
    "sources",
    "engine",
  ];
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
  const finish = () => save(answersToSettings(answers, { ...settings, localModel, language }));
  const skip = () => save({ ...settings, language, setupDone: true });

  return (
    <div className="fixed inset-0 z-50 flex flex-col overflow-y-auto bg-slate-50 dark:bg-slate-950">
      <header className="flex items-center gap-3 px-8 py-5">
        <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-rail text-white">
          <Logo className="h-7 w-7" />
        </span>
        <div className="flex-1">
          <div className="font-semibold">{t("wizard.title")}</div>
          <div className="text-xs text-slate-500">{t("wizard.progress", { step: index + 1, steps: steps.length })}</div>
        </div>
        <button className="btn btn-ghost" onClick={skip} disabled={busy}>
          {t("common.skip")}
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

        {step === "language" && (
          <Question title={t("wizard.language.title")} text={t("wizard.language.text")}>
            <div className="grid grid-cols-4 gap-2">
              {LANGUAGES.map(({ code, name, english, Flag }) => (
                <Tile
                  key={code}
                  media={<Flag className="h-5 w-7 shrink-0 rounded-sm shadow-sm ring-1 ring-black/10" />}
                  label={name}
                  hint={english === name ? undefined : english}
                  compact
                  selected={language === code}
                  onClick={() => setLanguage(code)}
                />
              ))}
            </div>
          </Question>
        )}

        {step === "interests" && (
          <Question title={t("wizard.interests.title")} text={t("wizard.interests.text")}>
            <Tiles>
              {(
                [
                  ["tcg", t("wizard.interests.tcg"), t("wizard.interests.tcgHint"), Layers],
                  ["sport", t("wizard.interests.sport"), t("wizard.interests.sportHint"), Trophy],
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
          <Question title={t("wizard.games.title")} text={t("wizard.games.text")}>
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
          <Question title={t("wizard.goals.title")} text={t("wizard.goals.text")}>
            <Tiles>
              <Tile
                icon={Search}
                label={t("wizard.goals.identification")}
                hint={t("wizard.goals.identificationHint")}
                selected
                locked
              />
              {(
                [
                  ["grading", t("wizard.goals.grading"), t("wizard.goals.gradingHint"), BadgeCheck],
                  ["condition", t("wizard.goals.condition"), t("wizard.goals.conditionHint"), Gauge],
                  ["prices", t("wizard.goals.prices"), t("wizard.goals.pricesHint"), DollarSign],
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
          <Question title={t("wizard.sources.title")} text={t("wizard.sources.text")}>
            <Tiles>
              {(
                [
                  ["scanner", t("wizard.sources.scanner"), t("wizard.sources.scannerHint"), ScanLine],
                  ["folder", t("wizard.sources.folder"), t("wizard.sources.folderHint"), FolderOpen],
                  ["watch_folder", t("wizard.sources.watchFolder"), t("wizard.sources.watchFolderHint"), Eye],
                  ["csv", t("wizard.sources.csv"), t("wizard.sources.csvHint"), FileSpreadsheet],
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
          <Question title={t("wizard.engine.title")}>
            <Tiles>
              <Tile
                icon={Sparkles}
                label={t("wizard.engine.ximilar")}
                hint={t("wizard.engine.ximilarHint")}
                selected={answers.engine === "ximilar"}
                onClick={() => setAnswers((a) => ({ ...a, engine: "ximilar" }))}
              />
              <Tile
                icon={Cpu}
                label={t("wizard.engine.local")}
                hint={t("wizard.engine.localHint")}
                selected={answers.engine === "local"}
                onClick={() => setAnswers((a) => ({ ...a, engine: "local" }))}
              />
            </Tiles>
            {answers.engine === "ximilar" ? (
              <div className="mt-6">
                <label className="label">{t("wizard.engine.token")}</label>
                <input
                  className="input"
                  type="password"
                  value={token}
                  placeholder={settings.tokenSet ? t("wizard.engine.tokenSaved") : t("wizard.engine.tokenPaste")}
                  onChange={(e) => setToken(e.target.value)}
                />
                <p className="mt-1.5 text-xs text-slate-500">
                  {rich(t("wizard.engine.tokenHelp"), {
                    link: (text) => (
                      <button className="text-accent underline" onClick={() => openUrl("https://app.ximilar.com")}>
                        {text}
                      </button>
                    ),
                  })}
                </p>
              </div>
            ) : (
              <div className="mt-6 space-y-3">
                {answers.goals.length > 0 && (
                  <p className="text-sm text-amber-700 dark:text-amber-400">
                    {[
                      answers.goals.includes("prices") && t("wizard.engine.pricesNeedApi"),
                      answers.goals.some((g) => g !== "prices") && t("wizard.engine.roughGrades"),
                    ]
                      .filter(Boolean)
                      .join(" ")}
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
              <ArrowLeft className="h-4 w-4" /> {t("common.back")}
            </button>
            {last ? (
              <button className="btn btn-primary" onClick={finish} disabled={busy}>
                {t("common.finish")} <CheckCircle2 className="h-4 w-4" />
              </button>
            ) : (
              <button className="btn btn-primary" onClick={() => setStep(steps[index + 1])} disabled={missing}>
                {t("common.next")} <ArrowRight className="h-4 w-4" />
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
  media,
  label,
  hint,
  selected,
  locked,
  compact,
  onClick,
}: {
  icon?: LucideIcon;
  /** Shown in place of an icon, e.g. a flag. */
  media?: ReactNode;
  label: string;
  hint?: string;
  selected: boolean;
  locked?: boolean;
  compact?: boolean;
  onClick?: () => void;
}) {
  const t = useT();
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
      {media && <span className="mt-0.5 flex">{media}</span>}
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
        {locked && <span className="mt-0.5 block text-xs text-slate-400">{t("wizard.alwaysIncluded")}</span>}
      </span>
      {selected ? (
        <CheckCircle2 className="h-5 w-5 shrink-0 text-accent" />
      ) : (
        <Circle className="h-5 w-5 shrink-0 text-slate-300 dark:text-slate-600" />
      )}
    </button>
  );
}
