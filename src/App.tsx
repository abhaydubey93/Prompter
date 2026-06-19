/** Main window — Settings + Prompt Library + History. */
import { useEffect, useState } from "react";
import {
  cmd, onHotkeyError,
  type Prompt, type Settings, type HistoryEntry,
} from "./lib/tauri";
import {
  Settings as SettingsIcon, BookOpen, History, Zap, Trash2, Search, AlertTriangle,
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

  useEffect(() => {
    loadLibrary();
    loadSettings();
    checkOnboarding();
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
    const setup = async () => {
      const unlisten = await onHotkeyError((ev) => {
        setToast(`Hotkey "${ev.shortcut}" conflict: ${ev.message}`);
        setTimeout(() => setToast(null), 5000);
      });
      return () => unlisten();
    };
    setup();
  }, []);

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

  useEffect(() => { if (tab === "history") loadHistory(); }, [tab]);

  const updateSetting = async (key: string, value: string) => {
    await cmd.setSetting(key, value);
    loadSettings();
  };

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
          <h1 className="text-base font-bold tracking-tight">PromptOpt</h1>
        </div>
        <nav className="flex-1 py-2">
          <SidebarItem icon={<BookOpen size={16} />} label="Prompt Library" active={tab === "library"} onClick={() => setTab("library")} />
          <SidebarItem icon={<History size={16} />} label="History" active={tab === "history"} onClick={() => setTab("history")} />
          <SidebarItem icon={<SettingsIcon size={16} />} label="Settings" active={tab === "settings"} onClick={() => setTab("settings")} />
        </nav>
        <div className="px-4 py-3 text-[10px] text-gray-600 border-t border-bg-600">
          PromptOpt v0.1.0 MVP
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
                    <button onClick={() => handleDelete(p.id)} className="p-1 text-gray-500 hover:text-red-400 shrink-0">
                      <Trash2 size={14} />
                    </button>
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
          <div className="p-6 max-w-xl">
            <h2 className="text-lg font-semibold mb-4">Settings</h2>
            <SettingRow label="Hotkey" value={settings.hotkey} onChange={(v) => updateSetting("hotkey", v)} />
            <SettingRow label="Theme" value={settings.theme} onChange={(v) => updateSetting("theme", v)} />
            <SettingRow label="Default Framework" value={settings.default_framework} onChange={(v) => updateSetting("default_framework", v)} />
            <SettingRow label="Default Model" value={settings.default_model} onChange={(v) => updateSetting("default_model", v)} />
            <SettingRow label="Ollama URL" value={settings.ollama_url} onChange={(v) => updateSetting("ollama_url", v)} />

            <div className="mt-8 p-4 bg-bg-700 rounded-lg border border-bg-600">
              <h3 className="text-sm font-semibold mb-2">Keyboard Shortcuts</h3>
              <KbdRow keys={["Ctrl", "Shift", "E"]} action="Open overlay / optimize selected text" />
              <KbdRow keys={["Enter"]} action="Accept and replace in-place" />
              <KbdRow keys={["Esc"]} action="Close overlay" />
            </div>
          </div>
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
      className={`flex items-center gap-3 px-4 py-2.5 text-sm transition ${
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
