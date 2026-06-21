/** Main window — Settings + Prompt Library + History. */
import { useEffect, useState } from "react";
import {
  cmd, onHotkeyError,
  type Prompt, type Settings, type HistoryEntry, type ProviderConfig, type FrameworkInfo, type ContextProfile,
} from "./lib/tauri";
import {
  Settings as SettingsIcon, BookOpen, History, Zap, Trash2, Search, AlertTriangle, Copy, Loader2,
} from "lucide-react";
import Onboarding from "./Onboarding";

type Tab = "library" | "history" | "settings";

export default function App() {
  const [tab, setTab] = useState<Tab>("library");
  const [prompts, setPrompts] = useState<Prompt[]>([]);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [search, setSearch] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([loadLibrary(), loadSettings(), checkOnboarding()])
      .finally(() => setLoading(false));
  }, []);

  // First-run / no-provider gate: show wizard if not completed or no provider enabled.
  async function checkOnboarding() {
    try {
      const st = await cmd.getOnboardingState();
      if (!st.completed || !st.has_enabled_provider) {
        setShowOnboarding(true);
      }
    } catch { /* ignore */ }
  }

  // Listen for hotkey conflict errors (spec §11 GAP-7).
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    onHotkeyError((ev) => {
      setToast(`Hotkey "${ev.shortcut}" conflict: ${ev.message}`);
      setTimeout(() => setToast(null), 5000);
    }).then((un) => {
      if (cancelled) {
        un();
      } else {
        unlisten = un;
      }
    });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  // Apply theme (WP-F): toggles data-theme attr on <html> for light/dark CSS.
  useEffect(() => {
    if (!settings) return;

    const updateTheme = () => {
      let resolvedTheme = "dark";
      if (settings.theme === "light") {
        resolvedTheme = "light";
      } else if (settings.theme === "system") {
        resolvedTheme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      }
      document.documentElement.setAttribute("data-theme", resolvedTheme);
    };

    updateTheme();

    if (settings.theme === "system") {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => updateTheme();
      mediaQuery.addEventListener("change", handler);
      return () => mediaQuery.removeEventListener("change", handler);
    }
  }, [settings?.theme]);

  const loadLibrary = async () => {
    try { setPrompts(await cmd.listPrompts()); } catch { /* ignore */ }
  };

  const loadSettings = async () => {
    try { setSettings(await cmd.getSettings()); } catch { /* ignore */ }
  };

  const loadHistory = async () => {
    try { setHistory(await cmd.listHistory(50)); } catch { /* ignore */ }
  };

  const handleSearch = async () => {
    if (!search.trim()) { loadLibrary(); return; }
    try { setPrompts(await cmd.searchPrompts(search)); } catch { /* ignore */ }
  };

  const handleDelete = async (id: string) => {
    try { await cmd.deletePrompt(id); setPrompts((p) => p.filter((x) => x.id !== id)); } catch { /* ignore */ }
  };

  const handleCopy = async (p: Prompt) => {
    try {
      await navigator.clipboard.writeText(p.body);
      await cmd.bumpUsage(p.id);
      setToast("Prompt copied to clipboard!");
      setTimeout(() => setToast(null), 3000);
      loadLibrary();
    } catch { /* ignore */ }
  };

  useEffect(() => { if (tab === "history") loadHistory(); }, [tab]);

  const updateSetting = async (key: keyof Settings, value: string) => {
    await cmd.setSetting(key, value);
    loadSettings();
  };

  if (loading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-bg-900 text-gray-200 select-none">
        <div className="flex flex-col items-center gap-3">
          <Loader2 className="animate-spin text-accent" size={32} />
          <span className="text-xs text-gray-500 font-medium tracking-wide">Loading Prompter...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-bg-900 text-gray-200">
      {/* ── Onboarding wizard (first-run / no provider) ──────────── */}
      {showOnboarding && <Onboarding onClose={() => setShowOnboarding(false)} />}

      {/* ── Toast (hotkey conflict, spec §11 GAP-7) ─────────────── */}
      {toast && (
        <div className="fixed top-4 right-4 z-50 flex items-center gap-2 px-4 py-3 bg-red-900/90 border border-red-700 rounded-lg text-red-200 text-sm shadow-overlay">
          <AlertTriangle size={16} />
          <span className="flex-1">{toast}</span>
          <button onClick={() => setToast(null)} className="hover:text-white text-xs">✕</button>
        </div>
      )}
      {/* ── Sidebar ──────────────────────────────────────────────── */}
      <aside className="w-56 bg-bg-800 border-r border-bg-600 flex flex-col shrink-0">
        <div className="px-4 py-4 flex items-center gap-2 border-b border-bg-600">
          <Zap size={20} className="text-accent" />
          <h1 className="text-base font-bold tracking-tight">Prompter</h1>
        </div>
        <nav className="flex-1 py-2">
          <SidebarItem icon={<BookOpen size={16} />} label="Prompt Library" active={tab === "library"} onClick={() => setTab("library")} />
          <SidebarItem icon={<History size={16} />} label="History" active={tab === "history"} onClick={() => setTab("history")} />
          <SidebarItem icon={<SettingsIcon size={16} />} label="Settings" active={tab === "settings"} onClick={() => setTab("settings")} />
        </nav>
        <div className="px-4 py-3 text-[10px] text-gray-600 border-t border-bg-600">
          Prompter v0.1.0 MVP
        </div>
      </aside>

      {/* ── Content ───────────────────────────────────────────────── */}
      <main className="flex-1 overflow-auto">
        {tab === "library" && (
          <div className="p-6 max-w-3xl">
            <div className="flex items-center gap-3 mb-4">
              <h2 className="text-lg font-semibold">Prompt Library</h2>
              <div className="flex-1 flex items-center gap-2 max-w-xs ml-auto">
                <input
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  placeholder="Search prompts…"
                  className="flex-1 bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs focus:outline-none focus:border-accent"
                />
                <button onClick={handleSearch} className="p-1.5 text-gray-400 hover:text-gray-200"><Search size={14} /></button>
              </div>
            </div>
            {prompts.length === 0 && (
              <p className="text-gray-500 text-sm">No prompts saved yet. Use the overlay to optimize and save prompts.</p>
            )}
            <div className="space-y-2">
              {prompts.map((p) => (
                <div key={p.id} className="bg-bg-700 rounded-lg p-3 border border-bg-600">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium truncate">{p.title}</div>
                      <div className="text-xs text-gray-400 mt-1 line-clamp-2 whitespace-pre-wrap">{p.body}</div>
                      <div className="flex items-center gap-2 mt-2 text-[10px] text-gray-500">
                        {p.framework && <span className="bg-bg-600 px-1.5 py-0.5 rounded">{p.framework}</span>}
                        {p.score > 0 && <span>Score: {p.score}</span>}
                        <span>Used: {p.usage_count}×</span>
                      </div>
                    </div>
                    <div className="flex gap-1 shrink-0">
                      <button onClick={() => handleCopy(p)} className="p-1 text-gray-500 hover:text-gray-200" title="Copy prompt and use">
                        <Copy size={14} />
                      </button>
                      <button onClick={() => handleDelete(p.id)} className="p-1 text-gray-500 hover:text-red-400">
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {tab === "history" && (
          <div className="p-6 max-w-3xl">
            <h2 className="text-lg font-semibold mb-4">Optimization History</h2>
            {history.length === 0 && <p className="text-gray-500 text-sm">No history yet.</p>}
            <div className="space-y-2">
              {history.map((h) => (
                <div key={h.id} className="bg-bg-700 rounded-lg p-3 border border-bg-600">
                  <div className="text-xs text-gray-400 mb-1">{h.timestamp} · {h.model}</div>
                  <div className="text-xs text-gray-500 line-clamp-1">{h.raw_prompt}</div>
                  <div className="text-sm text-gray-300 mt-1 line-clamp-2 whitespace-pre-wrap">{h.optimized_prompt}</div>
                  {h.score != null && <div className="text-[10px] text-gray-500 mt-1">Score: {h.score}/100</div>}
                </div>
              ))}
            </div>
          </div>
        )}

        {tab === "settings" && settings && (
          <SettingsTabs
            settings={settings}
            onSetting={updateSetting}
            onToast={(msg) => {
              setToast(msg);
              setTimeout(() => setToast(null), 5000);
            }}
          />
        )}
      </main>
    </div>
  );
}

// ── Sub-components ────────────────────────────────────────────────────────

function SidebarItem({ icon, label, active, onClick }: {
  icon: React.ReactNode; label: string; active: boolean; onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-4 py-2.5 text-sm transition ${
        active ? "bg-bg-600/50 text-white border-l-2 border-accent" : "text-gray-400 hover:bg-bg-700 hover:text-gray-200 border-l-2 border-transparent"
      }`}
    >
      {icon}
      {label}
    </button>
  );
}

function SettingRow({ label, value, onChange }: {
  label: string; value: string; onChange: (v: string) => void;
}) {
  return (
    <div className="flex items-center justify-between py-3 border-b border-bg-700">
      <span className="text-sm text-gray-400">{label}</span>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs w-56 focus:outline-none focus:border-accent"
      />
    </div>
  );
}

function KbdRow({ keys, action }: { keys: string[]; action: string }) {
  return (
    <div className="flex items-center gap-3 py-1 text-xs">
      <div className="flex gap-1">
        {keys.map((k) => (
          <kbd key={k} className="bg-bg-600 px-1.5 py-0.5 rounded text-[10px] font-mono">{k}</kbd>
        ))}
      </div>
      <span className="text-gray-500">{action}</span>
    </div>
  );
}

// ── Settings — 5 sub-tabs (use-case §15) ──────────────────────────────────

type SettingsTab = "general" | "providers" | "frameworks" | "context" | "privacy";

function SettingsTabs({ settings, onSetting, onToast }: {
  settings: Settings;
  onSetting: (k: keyof Settings, v: string) => void;
  onToast: (msg: string) => void;
}) {
  const [stab, setStab] = useState<SettingsTab>("general");
  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "general", label: "General" },
    { id: "providers", label: "Providers" },
    { id: "frameworks", label: "Frameworks" },
    { id: "context", label: "Context" },
    { id: "privacy", label: "Privacy" },
  ];

  return (
    <div className="p-6 max-w-2xl">
      <h2 className="text-lg font-semibold mb-4">Settings</h2>
      <div className="flex gap-1 mb-4 border-b border-bg-700">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setStab(t.id)}
            className={`px-3 py-2 text-xs border-b-2 ${
              stab === t.id ? "border-accent text-white" : "border-transparent text-gray-500 hover:text-gray-300"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {stab === "general" && (
        <GeneralTab settings={settings} onSetting={onSetting} />
      )}
      {stab === "providers" && <ProvidersTab />}
      {stab === "frameworks" && <FrameworksTab defaultFramework={settings.default_framework} onFramework={(v) => onSetting("default_framework", v)} onToast={onToast} />}
      {stab === "context" && <ContextTab />}
      {stab === "privacy" && <PrivacyTab onToast={onToast} />}
    </div>
  );
}

function GeneralTab({ settings, onSetting }: {
  settings: Settings;
  onSetting: (k: keyof Settings, v: string) => void;
}) {
  const [frameworks, setFrameworks] = useState<FrameworkInfo[]>([]);
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  useEffect(() => {
    cmd.listFrameworks().then(setFrameworks).catch(() => {});
    cmd.listProviders().then(setProviders).catch(() => {});
  }, []);
  return (
    <div>
      <SettingRow label="Hotkey" value={settings.hotkey} onChange={(v) => onSetting("hotkey", v)} />
      <div className="flex items-center justify-between py-3 border-b border-bg-700">
        <span className="text-sm text-gray-400">Theme</span>
        <select
          value={settings.theme}
          onChange={(e) => onSetting("theme", e.target.value)}
          className="bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs w-56"
        >
          <option value="dark">dark</option>
          <option value="light">light</option>
          <option value="system">system</option>
        </select>
      </div>
      <div className="flex items-center justify-between py-3 border-b border-bg-700">
        <span className="text-sm text-gray-400">Default Framework</span>
        <select
          value={settings.default_framework}
          onChange={(e) => onSetting("default_framework", e.target.value)}
          className="bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs w-56"
        >
          {frameworks.map((f) => (
            <option key={f.id} value={f.id}>{f.name}</option>
          ))}
        </select>
      </div>
      <div className="flex items-center justify-between py-3 border-b border-bg-700">
        <span className="text-sm text-gray-400">Default Provider</span>
        <select
          value={settings.default_provider_id || ""}
          onChange={(e) => onSetting("default_provider_id", e.target.value)}
          className="bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs w-56"
        >
          {providers.map((p) => (
            <option key={p.id} value={p.id}>{p.label}</option>
          ))}
        </select>
      </div>
      <SettingRow label="Default Model" value={settings.default_model} onChange={(v) => onSetting("default_model", v)} />

      <div className="mt-8 p-4 bg-bg-700 rounded-lg border border-bg-600">
        <h3 className="text-sm font-semibold mb-2">Keyboard Shortcuts</h3>
        <KbdRow keys={["Ctrl", "Shift", "E"]} action="Open overlay / optimize selected text" />
        <KbdRow keys={["Enter"]} action="Accept and replace in-place" />
        <KbdRow keys={["Esc"]} action="Close overlay" />
      </div>
    </div>
  );
}

function ProvidersTab() {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [editing, setEditing] = useState<ProviderConfig | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [testing, setTesting] = useState<string | null>(null);
  const [testMsg, setTestMsg] = useState<Record<string, string>>({});
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const load = () => cmd.listProviders().then(setProviders).catch(() => {});
  useEffect(() => { load(); }, []);

  async function toggle(p: ProviderConfig, enabled: boolean) {
    await cmd.setProviderEnabled(p.id, enabled);
    load();
  }
  async function test(p: ProviderConfig) {
    setTesting(p.id);
    setTestMsg((m) => ({ ...m, [p.id]: "..." }));
    try {
      const r = await cmd.testProvider(p.id);
      setTestMsg((m) => ({ ...m, [p.id]: r.alive ? `OK — ${r.models.length} models` : `FAIL: ${r.error ?? "unreachable"}` }));
    } catch (e) {
      setTestMsg((m) => ({ ...m, [p.id]: `ERR: ${e}` }));
    } finally {
      setTesting(null);
    }
  }
  async function saveEdit() {
    if (!editing) return;
    await cmd.saveProvider(editing);
    if (apiKey) {
      await cmd.setProviderKey(editing.id, apiKey);
      setApiKey("");
    }
    setEditing(null);
    load();
  }
  async function del(id: string) {
    await cmd.deleteProvider(id);
    load();
  }

  return (
    <div>
      {providers.map((p) => (
        <div key={p.id} className="py-3 border-b border-bg-700">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm text-gray-200">{p.label}</div>
              <div className="text-xs text-gray-500">{p.kind} · {p.base_url}</div>
            </div>
            <div className="flex items-center gap-2">
              <input type="checkbox" checked={p.enabled} onChange={(e) => toggle(p, e.target.checked)} />
              <button onClick={() => test(p)} disabled={testing === p.id} className="text-xs px-2 py-1 bg-bg-700 rounded hover:bg-bg-600">Test</button>
              <button onClick={() => { setEditing({ ...p }); setApiKey(""); }} className="text-xs px-2 py-1 bg-bg-700 rounded hover:bg-bg-600">Edit</button>
              {confirmDeleteId === p.id ? (
                <div className="flex gap-1">
                  <button onClick={() => { del(p.id); setConfirmDeleteId(null); }} className="text-xs px-2 py-1 bg-red-800 text-white rounded hover:bg-red-700 font-bold">confirm</button>
                  <button onClick={() => setConfirmDeleteId(null)} className="text-xs px-2 py-1 bg-bg-600 rounded">cancel</button>
                </div>
              ) : (
                <button onClick={() => setConfirmDeleteId(p.id)} className="text-xs px-2 py-1 bg-bg-700 rounded hover:bg-red-900">Del</button>
              )}
            </div>
          </div>
          {testMsg[p.id] && <div className="text-xs mt-1 text-gray-400">{testMsg[p.id]}</div>}
        </div>
      ))}

      {editing && (
        <div className="mt-4 p-4 bg-bg-700 rounded-lg border border-bg-600 space-y-2">
          <div className="text-sm font-semibold">Edit {editing.label}</div>
          <input value={editing.label} onChange={(e) => setEditing({ ...editing, label: e.target.value })} placeholder="Label" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <input value={editing.base_url} onChange={(e) => setEditing({ ...editing, base_url: e.target.value })} placeholder="Base URL" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <input value={editing.default_model} onChange={(e) => setEditing({ ...editing, default_model: e.target.value })} placeholder="Default model (bare name)" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          {editing.api_key_slot && (
            <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="API key (write-only; stored in keychain)" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          )}
          <div className="flex gap-2">
            <button onClick={saveEdit} className="text-xs px-3 py-1 bg-emerald-700 rounded hover:bg-emerald-600">Save</button>
            <button onClick={() => setEditing(null)} className="text-xs px-3 py-1 bg-bg-600 rounded">Cancel</button>
          </div>
        </div>
      )}

      <button
        onClick={() => setEditing({ id: "", kind: "openai_compat", label: "New Provider", base_url: "https://", api_key_slot: "new", default_model: "", enabled: false, sort_order: 100 })}
        className="mt-4 text-xs px-3 py-1.5 bg-bg-700 rounded hover:bg-bg-600"
      >+ Add custom provider</button>
    </div>
  );
}

function FrameworksTab({ defaultFramework, onFramework, onToast }: {
  defaultFramework: string;
  onFramework: (v: string) => void;
  onToast: (msg: string) => void;
}) {
  const [frameworks, setFrameworks] = useState<FrameworkInfo[]>([]);
  const load = () => cmd.listFrameworks().then(setFrameworks).catch(() => {});
  useEffect(() => { load(); }, []);

  function importJson() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const text = await file.text();
      try {
        const pack = JSON.parse(text);
        await cmd.importFramework({ ...pack, template: pack.template ?? "{{ raw_prompt }}" });
        load();
        onToast("Framework imported successfully.");
      } catch (e) {
        onToast(`Import failed: ${e}`);
      }
    };
    input.click();
  }
  async function del(id: string) {
    try {
      await cmd.deleteFramework(id);
      load();
      onToast("Framework deleted.");
    } catch (e) {
      onToast(`Cannot delete (likely built-in): ${e}`);
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between py-3 border-b border-bg-700">
        <span className="text-sm text-gray-400">Default Framework</span>
        <select
          value={defaultFramework}
          onChange={(e) => onFramework(e.target.value)}
          className="bg-bg-700 border border-bg-600 rounded px-3 py-1.5 text-xs w-56"
        >
          {frameworks.map((f) => (
            <option key={f.id} value={f.id}>{f.name}</option>
          ))}
        </select>
      </div>
      <div className="mt-4 space-y-2">
        {frameworks.map((f) => (
          <div key={f.id} className="flex items-center justify-between py-2 border-b border-bg-700">
            <span className="text-sm text-gray-200">{f.name}</span>
            <button onClick={() => del(f.id)} className="text-xs text-gray-500 hover:text-red-400">delete</button>
          </div>
        ))}
      </div>
      <button onClick={importJson} className="mt-4 text-xs px-3 py-1.5 bg-bg-700 rounded hover:bg-bg-600">+ Import JSON pack</button>
    </div>
  );
}

function ContextTab() {
  const [profiles, setProfiles] = useState<ContextProfile[]>([]);
  const [draft, setDraft] = useState<ContextProfile | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const load = () => cmd.listContexts().then(setProfiles).catch(() => {});
  useEffect(() => { load(); }, []);

  async function save() {
    if (!draft || !draft.name) return;
    await cmd.saveContext({ ...draft, id: draft.id || crypto.randomUUID() });
    setDraft(null);
    load();
  }

  async function remove(id: string) {
    await cmd.deleteContext(id);
    load();
  }

  return (
    <div>
      <div className="space-y-2">
        {profiles.map((c) => (
          <div key={c.id} className="flex items-center justify-between py-2 border-b border-bg-700">
            <div>
              <div className="text-sm text-gray-200">{c.name}</div>
              <div className="text-xs text-gray-500">{[c.role, c.audience, c.tone].filter(Boolean).join(" · ")}</div>
            </div>
            <div className="flex gap-2">
              <button onClick={() => setDraft({ ...c })} className="text-xs text-gray-500 hover:text-gray-300">edit</button>
              {confirmDeleteId === c.id ? (
                <>
                  <button onClick={() => { remove(c.id); setConfirmDeleteId(null); }} className="text-xs text-red-400 hover:text-red-300 font-bold">confirm</button>
                  <button onClick={() => setConfirmDeleteId(null)} className="text-xs text-gray-500 hover:text-gray-300">cancel</button>
                </>
              ) : (
                <button onClick={() => setConfirmDeleteId(c.id)} className="text-xs text-gray-500 hover:text-red-400">delete</button>
              )}
            </div>
          </div>
        ))}
      </div>
      {draft && (
        <div className="mt-4 p-4 bg-bg-700 rounded-lg border border-bg-600 space-y-2">
          <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Name" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <input value={draft.role ?? ""} onChange={(e) => setDraft({ ...draft, role: e.target.value })} placeholder="Role" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <input value={draft.audience ?? ""} onChange={(e) => setDraft({ ...draft, audience: e.target.value })} placeholder="Audience" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <input value={draft.tone ?? ""} onChange={(e) => setDraft({ ...draft, tone: e.target.value })} placeholder="Tone" className="w-full bg-bg-950 border border-bg-600 rounded px-2 py-1 text-xs" />
          <div className="flex gap-2">
            <button onClick={save} className="text-xs px-3 py-1 bg-emerald-700 rounded">Save</button>
            <button onClick={() => setDraft(null)} className="text-xs px-3 py-1 bg-bg-600 rounded">Cancel</button>
          </div>
        </div>
      )}
      <button onClick={() => setDraft({ id: "", name: "" })} className="mt-4 text-xs px-3 py-1.5 bg-bg-700 rounded hover:bg-bg-600">+ Add profile</button>
    </div>
  );
}

function PrivacyTab({ onToast }: { onToast: (msg: string) => void }) {
  const [telemetry, setTelemetry] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  useEffect(() => {
    cmd.getMeta("telemetry_enabled").then((v) => setTelemetry(v === "1")).catch(() => {});
  }, []);
  async function toggleTelemetry(on: boolean) {
    setTelemetry(on);
    await cmd.setMeta("telemetry_enabled", on ? "1" : "0");
  }
  async function clearHist() {
    try {
      await cmd.clearHistory();
      onToast("History cleared.");
    } catch (e) {
      onToast(`Failed to clear history: ${e}`);
    }
  }
  return (
    <div>
      <div className="flex items-center justify-between py-3 border-b border-bg-700">
        <div>
          <div className="text-sm text-gray-200">Anonymous telemetry</div>
          <div className="text-xs text-gray-500">Off by default. Helps improve prompt scoring.</div>
        </div>
        <input type="checkbox" checked={telemetry} onChange={(e) => toggleTelemetry(e.target.checked)} />
      </div>
      <div className="py-3 border-b border-bg-700">
        {confirmClear ? (
          <div className="flex items-center gap-2">
            <span className="text-xs text-red-400 font-medium">Are you sure you want to clear all history?</span>
            <button onClick={() => { clearHist(); setConfirmClear(false); }} className="text-xs px-3 py-1 bg-red-800 text-white rounded hover:bg-red-700 font-bold">Clear</button>
            <button onClick={() => setConfirmClear(false)} className="text-xs px-3 py-1 bg-bg-600 rounded">Cancel</button>
          </div>
        ) : (
          <button onClick={() => setConfirmClear(true)} className="text-xs px-3 py-1.5 bg-red-900/50 text-red-200 rounded hover:bg-red-900">Clear optimization history</button>
        )}
      </div>
      <div className="mt-4 text-xs text-gray-500">
        API keys are stored only in the OS keychain — never in the database or logs.
      </div>
    </div>
  );
}
