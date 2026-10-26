import clsx from "clsx";
import {
  Cloud,
  Eye,
  FileSpreadsheet,
  FileUp,
  FolderOpen,
  HardDrive,
  Plus,
  RefreshCw,
  ScanLine,
  Search,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import Menu from "../components/Menu";
import { ErrorNote } from "../components/Modal";
import { useRemoteSessionActions, useSessionActions } from "../components/SessionActions";
import NewSessionDialog from "../components/NewSessionDialog";
import StatusChip from "../components/StatusChip";
import { api, errorText, onSessionUpdated, type RemoteSession, type Session, type SourceKind } from "../lib/api";
import { basename, dateTime, price } from "../lib/format";
import Select, { type SelectOption } from "../components/Select";

const SOURCE_ICONS = { folder: FolderOpen, watch_folder: Eye, scanner: ScanLine, csv: FileSpreadsheet };

export default function SessionsPage() {
  const [params, setParams] = useSearchParams();
  const tab = params.get("tab") === "remote" ? "remote" : "local";
  const [creating, setCreating] = useState<SourceKind | null>(null);
  const [source, setSource] = useState<SourceKind>("folder");
  useEffect(() => {
    api.getSettings().then((s) => s.defaultSources[0] && setSource(s.defaultSources[0]));
  }, []);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-4 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <h1 className="text-lg font-semibold">Sessions</h1>
        <div className="flex rounded-md bg-slate-100 p-0.5 dark:bg-slate-900">
          {(
            [
              ["local", "Local", HardDrive],
              ["remote", "Cards Cloud", Cloud],
            ] as const
          ).map(([key, label, Icon]) => (
            <button
              key={key}
              onClick={() => setParams(key === "local" ? {} : { tab: key })}
              className={clsx(
                "flex items-center gap-1.5 rounded px-3 py-1 text-sm font-medium",
                tab === key
                  ? "bg-white shadow-xs dark:bg-slate-800"
                  : "text-slate-500 hover:text-slate-800 dark:hover:text-slate-200",
              )}
            >
              <Icon className="h-4 w-4" />
              {label}
            </button>
          ))}
        </div>
        <div className="flex-1" />
        <button
          className="btn"
          onClick={() => setCreating("csv")}
          title="A new session from a CSV of cards or image paths"
        >
          <FileUp className="h-4 w-4" /> Import CSV
        </button>
        <button className="btn btn-primary" onClick={() => setCreating(source)}>
          <Plus className="h-4 w-4" /> New session
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">{tab === "local" ? <LocalSessions /> : <RemoteSessions />}</div>
      {creating && <NewSessionDialog initialKind={creating} onClose={() => setCreating(null)} />}
    </div>
  );
}

const STATUS_FILTERS: SelectOption<string>[] = [
  { value: "", label: "All statuses" },
  { value: "active", label: "Running or paused" },
  { value: "draft", label: "Draft" },
  { value: "completed", label: "Completed" },
  { value: "stopped", label: "Stopped" },
  { value: "failed", label: "Failed" },
];

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

function matchesStatus(s: Session, filter: string) {
  if (!filter) return true;
  if (filter === "active") return s.status === "running" || s.status === "paused";
  return s.status === filter;
}

function LocalSessions() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const actions = useSessionActions({
    onDeleted: (ids) => {
      setSessions((list) => list && list.filter((s) => !ids.includes(s.id)));
      setChecked((c) => new Set([...c].filter((id) => !ids.includes(id))));
    },
  });

  useEffect(() => {
    api
      .listSessions()
      .then(setSessions)
      .catch((e) => setError(errorText(e)));
    const unlisten = onSessionUpdated((s) =>
      setSessions((list) => {
        if (!list) return list;
        const i = list.findIndex((x) => x.id === s.id);
        if (i < 0) return [s, ...list];
        const next = [...list];
        next[i] = s;
        return next;
      }),
    );
    return () => void unlisten.then((f) => f());
  }, []);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (sessions ?? []).filter(
      (s) =>
        matchesStatus(s, status) && (!q || s.name.toLowerCase().includes(q) || s.sourceRef.toLowerCase().includes(q)),
    );
  }, [sessions, query, status]);

  if (error)
    return (
      <div className="p-6">
        <ErrorNote>{error}</ErrorNote>
      </div>
    );
  if (!sessions) return null;
  if (sessions.length === 0)
    return (
      <Empty
        title="No sessions yet"
        text="Create a session to analyse a folder of card images or a stack of cards in your scanner."
      />
    );

  const picked = shown.filter((s) => checked.has(s.id));
  const allChecked = shown.length > 0 && picked.length === shown.length;
  const toggle = (id: string) =>
    setChecked((c) => {
      const n = new Set(c);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 px-6 py-3">
        {picked.length > 0 ? (
          <>
            <span className="text-sm font-medium">{picked.length} selected</span>
            <button className="btn btn-ghost text-red-600 dark:text-red-400" onClick={() => actions.remove(picked)}>
              <Trash2 className="h-4 w-4" /> Delete
            </button>
            <button className="btn btn-ghost" onClick={() => setChecked(new Set())}>
              Clear
            </button>
          </>
        ) : (
          <>
            <SearchBox value={query} onChange={setQuery} placeholder="Search sessions" />
            <div className="w-48">
              <Select value={status} onChange={setStatus} options={STATUS_FILTERS} aria-label="Status" />
            </div>
          </>
        )}
        <span className="flex-1" />
        <span className="text-xs text-slate-500">
          {shown.length === sessions.length
            ? plural(sessions.length, "session")
            : `${shown.length} of ${sessions.length}`}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <table className="w-full text-sm">
          <thead className="sticky top-0 z-10 bg-white text-left text-xs text-slate-500 uppercase dark:bg-slate-950">
            <tr className="border-b border-slate-200 dark:border-slate-800">
              <th className="w-10 py-2 pr-1 pl-6">
                <input
                  type="checkbox"
                  className="h-4 w-4 accent-[var(--color-accent)]"
                  checked={allChecked}
                  onChange={() => setChecked(allChecked ? new Set() : new Set(shown.map((s) => s.id)))}
                  aria-label="Select all"
                />
              </th>
              <th className="px-3 py-2 font-medium">Name</th>
              <th className="px-3 py-2 font-medium">Source</th>
              <th className="px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2 text-right font-medium">Cards</th>
              <th className="px-3 py-2 text-right font-medium">Value</th>
              <th className="px-3 py-2 font-medium">Created</th>
              <th className="px-3 py-2 font-medium">Cards Cloud</th>
              <th className="w-12 pr-4" />
            </tr>
          </thead>
          <tbody>
            {shown.map((s) => {
              const Icon = SOURCE_ICONS[s.sourceKind];
              const open = () => navigate(`/sessions/${s.id}`);
              return (
                <tr
                  key={s.id}
                  onClick={open}
                  className={clsx(
                    "group cursor-pointer border-b border-slate-100 dark:border-slate-900",
                    checked.has(s.id) ? "bg-accent/5 dark:bg-accent/10" : "hover:bg-slate-50 dark:hover:bg-slate-900",
                  )}
                >
                  <td className="py-2.5 pr-1 pl-6" onClick={(e) => e.stopPropagation()}>
                    <input
                      type="checkbox"
                      className="h-4 w-4 accent-[var(--color-accent)]"
                      checked={checked.has(s.id)}
                      onChange={() => toggle(s.id)}
                      aria-label={`Select ${s.name}`}
                    />
                  </td>
                  <td className="max-w-72 truncate px-3 py-2.5 font-medium" title={s.name}>
                    {s.name}
                  </td>
                  <td className="px-3 py-2.5 text-slate-500">
                    <span className="flex items-center gap-1.5" title={s.sourceRef}>
                      <Icon className="h-4 w-4 shrink-0" />
                      <span className="max-w-56 truncate">
                        {s.sourceKind === "scanner" ? s.sourceRef : basename(s.sourceRef)}
                      </span>
                    </span>
                  </td>
                  <td className="px-3 py-2.5">
                    <StatusChip status={s.status} />
                  </td>
                  <td className="px-3 py-2.5 text-right tabular-nums">
                    {s.counts.done}
                    <span className="text-slate-400"> / {s.counts.total}</span>
                  </td>
                  <td className="px-3 py-2.5 text-right tabular-nums">{price(s.counts.totalPrice || null)}</td>
                  <td className="px-3 py-2.5 whitespace-nowrap text-slate-500">{dateTime(s.createdAt)}</td>
                  <td className="px-3 py-2.5 text-slate-500">
                    {s.remoteSessionId ? (
                      <span className="flex items-center gap-1 text-xs whitespace-nowrap">
                        <Cloud className="h-3.5 w-3.5" /> {s.counts.synced} synced
                      </span>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className="pr-4 text-right" onClick={(e) => e.stopPropagation()}>
                    <Menu items={actions.menu(s, { open })} label={`Actions for ${s.name}`} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {shown.length === 0 && <p className="p-10 text-center text-sm text-slate-500">No sessions match.</p>}
      </div>
      {actions.node}
    </div>
  );
}

function SearchBox({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <div className="relative w-64">
      <Search className="pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2 text-slate-400" />
      <input
        className="input pl-8"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        onKeyDown={(e) => e.key === "Escape" && onChange("")}
      />
    </div>
  );
}

function RemoteSessions() {
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{ results: RemoteSession[]; count: number; next: string | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [reload, setReload] = useState(0);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  const actions = useRemoteSessionActions({
    onRenamed: (r) =>
      setData((d) => d && { ...d, results: d.results.map((s) => (s.id === r.id ? { ...s, name: r.name } : s)) }),
    onDeleted: () => load(),
  });

  // Searching asks the server, a moment after the last keystroke.
  useEffect(() => {
    const t = setTimeout(() => {
      setSearch(query);
      setPage(1);
    }, 350);
    return () => clearTimeout(t);
  }, [query]);

  useEffect(() => {
    let alive = true;
    api
      .listRemoteSessions(page, search)
      .then((d) => {
        if (!alive) return;
        setData(d);
        setError(null);
      })
      .catch((e) => alive && setError(errorText(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [page, search, reload]);

  const load = () => {
    setLoading(true);
    setReload((r) => r + 1);
  };

  if (error)
    return (
      <div className="space-y-3 p-6">
        <ErrorNote>{error}</ErrorNote>
        <p className="text-sm text-slate-500">
          Remote sessions live in Ximilar Cards Cloud. Check your API token in{" "}
          <Link to="/settings" className="text-accent underline">
            Settings
          </Link>
          ; the account needs the Cards Cloud service.
        </p>
      </div>
    );
  if (!data) return <div className="p-6 text-slate-500">{loading ? "Loading…" : null}</div>;
  if (data.results.length === 0 && !search)
    return (
      <Empty
        title="No sessions in Cards Cloud yet"
        text="Turn on “Sync to Ximilar Cards Cloud” on a session to upload it."
      />
    );

  return (
    <div>
      <div className="flex items-center gap-2 px-6 py-3">
        <SearchBox value={query} onChange={setQuery} placeholder="Search Cards Cloud" />
      </div>
      <table className="w-full text-sm">
        <thead className="sticky top-0 bg-white text-left text-xs text-slate-500 uppercase dark:bg-slate-950">
          <tr className="border-b border-slate-200 dark:border-slate-800">
            <th className="px-6 py-2 font-medium">Name</th>
            <th className="px-3 py-2 font-medium">Status</th>
            <th className="px-3 py-2 font-medium">Client</th>
            <th className="px-3 py-2 text-right font-medium">Cards</th>
            <th className="px-3 py-2 text-right font-medium">Value</th>
            <th className="px-3 py-2 font-medium">Started</th>
            <th className="w-12 pr-4" />
          </tr>
        </thead>
        <tbody>
          {data.results.map((s) => (
            <tr
              key={s.id}
              onClick={() => navigate(`/remote/${s.id}`)}
              className="cursor-pointer border-b border-slate-100 hover:bg-slate-50 dark:border-slate-900 dark:hover:bg-slate-900"
            >
              <td className="px-6 py-2.5 font-medium">{s.name || "Untitled"}</td>
              <td className="px-3 py-2.5">
                <StatusChip status={s.status} />
              </td>
              <td className="px-3 py-2.5 text-slate-500">{s.client?.name ?? s.platform}</td>
              <td className="px-3 py-2.5 text-right tabular-nums">{s.identification_count}</td>
              <td className="px-3 py-2.5 text-right tabular-nums">{price(s.total_price || null)}</td>
              <td className="px-3 py-2.5 whitespace-nowrap text-slate-500">{dateTime(s.started)}</td>
              <td className="pr-4 text-right" onClick={(e) => e.stopPropagation()}>
                <Menu
                  items={actions.menu(s, { open: () => navigate(`/remote/${s.id}`) })}
                  label={`Actions for ${s.name || "Untitled"}`}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {data.results.length === 0 && <p className="p-10 text-center text-sm text-slate-500">No sessions match.</p>}
      <div className="flex items-center justify-between px-6 py-3 text-sm text-slate-500">
        <span>{plural(data.count, "session")}</span>
        {actions.node}
        <div className="flex gap-2">
          <button className="btn btn-ghost" onClick={load} disabled={loading}>
            <RefreshCw className={clsx("h-4 w-4", loading && "animate-spin")} />
          </button>
          <button className="btn" disabled={page <= 1} onClick={() => (setLoading(true), setPage(page - 1))}>
            Previous
          </button>
          <button className="btn" disabled={!data.next} onClick={() => (setLoading(true), setPage(page + 1))}>
            Next
          </button>
        </div>
      </div>
    </div>
  );
}

function Empty({ title, text }: { title: string; text: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 p-10 text-center">
      <Layers2 />
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="max-w-sm text-sm text-slate-500">{text}</p>
    </div>
  );
}

function Layers2() {
  return (
    <div className="mb-2 flex h-14 w-14 items-center justify-center rounded-2xl bg-accent/10 text-accent">
      <ScanLine className="h-7 w-7" />
    </div>
  );
}
