/** Overlay window root — renders the Prompter overlay UI. */
import { useEffect, useState, useRef, useCallback } from "react";
import {
  cmd, onOptChunk, onOptDone, onOptError, onOverlayShow, onProviderStatus,
  type FrameworkInfo, type ModelInfo, type Settings, type ContextProfile,
} from "../lib/tauri";
import {
  Sparkles, Check, Copy, GitCompare,
  Loader2, AlertTriangle, X, BookOpen, Zap,
} from "lucide-react";

export default function OverlayApp() {
  // ── State ────────────────────────────────────────────────────────────
  const [rawText, setRawText] = useState("");
  const [optimizedText, setOptimizedText] = useState("");
  const optimizedRef = useRef("");
  useEffect(() => { optimizedRef.current = optimizedText; }, [optimizedText]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [score, setScore] = useState<number | null>(null);
  const [diff, setDiff] = useState("");
  const [diffVisible, setDiffVisible] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [frameworks, setFrameworks] = useState<FrameworkInfo[]>([]);
  const [selectedFramework, setSelectedFramework] = useState("CREATE");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [_settings, setSettings] = useState<Settings | null>(null);
  const [activeProviderId, setActiveProviderId] = useState("");
  const [providerAlive, setProviderAlive] = useState(true);
  const [contexts, setContexts] = useState<ContextProfile[]>([]);
  const [selectedContextId, setSelectedContextId] = useState("");
  const [refineActive, setRefineActive] = useState(false);
  const [refineNotes, setRefineNotes] = useState("");
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false);
  const [modelSearch, setModelSearch] = useState("");

  const sessionIdRef = useRef("");
  const rawRef = useRef<HTMLTextAreaElement>(null);
  const optRef = useRef<HTMLTextAreaElement>(null);
  const refineInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setModelDropdownOpen(false);
      }
    };
    if (modelDropdownOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [modelDropdownOpen]);

  useEffect(() => {
    if (refineActive) {
      refineInputRef.current?.focus();
    }
  }, [refineActive]);

  const activeProviderIdRef = useRef(activeProviderId);
  useEffect(() => {
    activeProviderIdRef.current = activeProviderId;
  }, [activeProviderId]);

  // ── Init ─────────────────────────────────────────────────────────────
  useEffect(() => {
    (async () => {
      try {
        // Pull text buffered at hotkey time FIRST — fixes emit/show race
        // where overlay_show fired before this React listener registered.
        const pending = await cmd.takePendingText();
        if (pending) setRawText(pending);

        const [fw, st, ctxs] = await Promise.all([cmd.listFrameworks(), cmd.getSettings(), cmd.listContexts()]);
        setFrameworks(fw);
        setSettings(st);
        setContexts(ctxs);
        setSelectedFramework(st.default_framework);
        setSelectedModel(st.default_model);

        // Load models for the default provider (or ollama fallback).
        const providerId = st.default_provider_id || "ollama";
        setActiveProviderId(providerId);
        try {
          const ms = await cmd.getModels(providerId);
          setModels(ms);
          // If no valid default_model, pick first from list.
          const hasValid = st.default_model && st.default_model.startsWith(providerId + ":");
          if (ms.length > 0 && !hasValid) {
            setSelectedModel(`${providerId}:${ms[0].id}`);
          }
        } catch {
          // Provider unreachable — models stay empty.
        }
      } catch (e: any) {
        console.error("init error", e);
      }
    })();

    // Listen for streaming events.
    type UnlistenFn = () => void;
    let cancelled = false;
    let cleaners: UnlistenFn[] = [];
    const setup = async () => {
      try {
        const [un0, un1, un2, un3, un4] = await Promise.all([
          // overlay_show event from hotkey handler (spec §7).
          onOverlayShow(async (ev) => {
            setOptimizedText("");
            setScore(null);
            setDiff("");
            setDiffVisible(false);
            setError(null);
            setIsStreaming(false);

            // Re-fetch settings and contexts on show to apply changes.
            try {
              const [st, ctxs] = await Promise.all([cmd.getSettings(), cmd.listContexts()]);
              setSettings(st);
              setContexts(ctxs);
              
              const providerId = st.default_provider_id || "ollama";
              setActiveProviderId(providerId);
              
              try {
                const res = await cmd.testProvider(providerId);
                setProviderAlive(res.alive);
                if (res.alive) {
                  setModels(res.models);
                  const hasValid = st.default_model && st.default_model.startsWith(providerId + ":");
                  if (res.models.length > 0 && !hasValid) {
                    setSelectedModel(`${providerId}:${res.models[0].id}`);
                  } else if (hasValid) {
                    setSelectedModel(st.default_model);
                  }
                } else {
                  setModels([]);
                }
              } catch (e) {
                setProviderAlive(false);
                setModels([]);
              }
            } catch (e) {
              console.error("Failed to reload settings/contexts on show:", e);
            }

            const text = ev.text || "";
            if (text) {
              setRawText(text);
            } else {
              try {
                const result = await cmd.captureText();
                setRawText(result.text);
              } catch {
                setRawText("");
              }
            }
          }),
          onOptChunk((ev) => {
            setOptimizedText((prev) => prev + ev.text);
          }),
          onOptDone((ev) => {
            setScore(ev.score);
            setDiff(ev.diff);
            setIsStreaming(false);
          }),
          onOptError((ev) => {
            setError(ev.message);
            setIsStreaming(false);
          }),
          // Provider health — only react to active provider.
          onProviderStatus((ev) => {
            if (ev.provider === activeProviderIdRef.current) setProviderAlive(ev.alive);
          }),
        ]);
        if (cancelled) {
          [un0, un1, un2, un3, un4].forEach((fn) => fn());
          return;
        }
        cleaners = [un0, un1, un2, un3, un4];
      } catch (e) {
        console.error("Failed to setup listeners:", e);
      }
    };

    setup();

    return () => {
      cancelled = true;
      cleaners.forEach((fn) => fn());
    };
  }, []);

  // Apply theme (WP-F): toggles data-theme attr on <html> for light/dark CSS.
  useEffect(() => {
    if (!_settings) return;

    const updateTheme = () => {
      let resolvedTheme = "dark";
      if (_settings.theme === "light") {
        resolvedTheme = "light";
      } else if (_settings.theme === "system") {
        resolvedTheme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      }
      document.documentElement.setAttribute("data-theme", resolvedTheme);
    };

    updateTheme();

    if (_settings.theme === "system") {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => updateTheme();
      mediaQuery.addEventListener("change", handler);
      return () => mediaQuery.removeEventListener("change", handler);
    }
  }, [_settings?.theme]);

  // ── Actions ───────────────────────────────────────────────────────────
  const handleOptimize = useCallback(async (notes?: string) => {
    if (!rawText.trim()) return;
    sessionIdRef.current = crypto.randomUUID();
    setOptimizedText("");
    setScore(null);
    setDiff("");
    setError(null);
    setIsStreaming(true);

    const notesToUse = typeof notes === "string" ? notes : refineNotes;

    let res;
    try {
      res = await cmd.optimizePrompt({
        raw: rawText,
        framework: selectedFramework,
        model: selectedModel,
        context_id: selectedContextId || undefined,
        refinement_notes: notesToUse || undefined,
      });
    } catch (e: any) {
      setError(String(e));
    } finally {
      // opt_done normally clears isStreaming, but emit/show races can drop it.
      // Ensure the button always unblocks.
      setIsStreaming(false);
    }

    // Safety net: if opt_done event was missed (StrictMode re-subscribe race,
    // window not yet listening, etc.), apply the result directly.
    if (res && res.optimized && !optimizedRef.current.trim()) {
      setOptimizedText(res.optimized);
      setScore(res.score);
      setDiff(res.diff);
    }
  }, [rawText, selectedFramework, selectedModel, selectedContextId, refineNotes]);

  const handleSubmitRefine = useCallback(async () => {
    if (!refineNotes.trim()) return;
    await handleOptimize(refineNotes);
    setRefineActive(false);
  }, [refineNotes, handleOptimize]);

  const handleRefineKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && refineNotes.trim()) {
      e.preventDefault();
      handleSubmitRefine();
    }
  }, [refineNotes, handleSubmitRefine]);

  const handleAccept = useCallback(async () => {
    if (!optimizedText.trim()) return;
    try {
      const result = await cmd.acceptReplacement(optimizedText);
      if (result.fallback) {
        // Clipboard fallback — tell user to paste.
        setError("Clipboard fallback: press Ctrl+V to paste enhanced text.");
      }
      // Success — hide overlay.
      cmd.hideOverlay().catch(() => {});
    } catch (e: any) {
      // Fallback: copy to clipboard and let user paste.
      try {
        await navigator.clipboard.writeText(optimizedText);
        setError("Replacement failed. Press Ctrl+V to paste.");
      } catch {
        setError("Replacement failed. Text copied — paste manually.");
      }
    }
  }, [optimizedText]);

  const handleSave = useCallback(async () => {
    if (!optimizedText.trim()) return;
    try {
      await cmd.savePrompt({
        id: crypto.randomUUID(),
        title: rawText.slice(0, 80),
        body: optimizedText,
        framework: selectedFramework,
        model_used: selectedModel,
        score: score ?? 0,
        usage_count: 1,
        created_at: new Date().toISOString(),
      });
    } catch (e: any) {
      console.error("save error", e);
    }
  }, [rawText, optimizedText, selectedFramework, selectedModel, score]);

  // ── Keyboard shortcuts inside overlay (spec §7) ───────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cmd.hideOverlay().catch(() => {});
      }

      // Ctrl+R = Toggle/open Refine input
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "r") {
        e.preventDefault();
        if (optimizedText.trim() && !isStreaming) {
          setRefineActive((a) => !a);
        }
      }

      // Ctrl+M = Focus Model dropdown
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "m") {
        e.preventDefault();
        setModelDropdownOpen(true);
      }

      // Ctrl+S = Save to library
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (optimizedText.trim() && !isStreaming) {
          handleSave();
        }
      }

      // Enter = Accept (spec §7) — but only when NOT focused on a text input
      // or textarea, so users can type newlines with Shift+Enter there.
      const el = document.activeElement;
      const inTextField = el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
      if (e.key === "Enter" && !e.shiftKey && !inTextField && optimizedText.trim() && !isStreaming) {
        e.preventDefault();
        handleAccept();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [optimizedText, isStreaming, handleAccept, handleSave]);

  // ── Score color ───────────────────────────────────────────────────────
  const scoreColor = score == null ? "text-gray-500"
    : score >= 70 ? "text-green-400"
    : score >= 40 ? "text-yellow-400"
    : "text-red-400";

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div
      className="flex flex-col h-screen bg-bg-900 text-gray-200 select-none"
      style={{ opacity: _settings?.overlay_opacity ? _settings.overlay_opacity / 100 : 0.9 }}
    >
      {/* ── Header ─────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-bg-600 gap-2 shrink-0"
        data-tauri-drag-region
      >
        <div className="flex items-center gap-2 text-sm">
          <Zap size={14} className="text-accent" />
          <select
            value={selectedFramework}
            onChange={(e) => setSelectedFramework(e.target.value)}
            className="bg-bg-700 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent min-w-[100px]"
          >
            {frameworks.map((f) => (
              <option key={f.id} value={f.id}>{f.name}</option>
            ))}
          </select>
          <div className="relative flex-1 min-w-[120px]" ref={dropdownRef}>
            <button
              onClick={() => setModelDropdownOpen(!modelDropdownOpen)}
              className="w-full text-left bg-bg-700 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent truncate"
            >
              {models.find(m => `${activeProviderId || "ollama"}:${m.id}` === selectedModel)?.name || selectedModel || "no models"}
            </button>
            {modelDropdownOpen && (
              <div className="absolute top-full left-0 mt-1 w-full max-h-60 bg-bg-800 border border-bg-600 rounded shadow-lg z-50 flex flex-col">
                <div className="p-1 border-b border-bg-600">
                  <input
                    type="text"
                    autoFocus
                    placeholder="Search model..."
                    value={modelSearch}
                    onChange={(e) => setModelSearch(e.target.value)}
                    className="w-full bg-bg-950 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent"
                  />
                </div>
                <div className="overflow-y-auto flex-1">
                  {models.filter(m => m.name.toLowerCase().includes(modelSearch.toLowerCase()) || m.id.toLowerCase().includes(modelSearch.toLowerCase())).map(m => {
                    const val = `${activeProviderId || "ollama"}:${m.id}`;
                    return (
                      <button
                        key={m.id}
                        className={`w-full text-left px-2 py-1 text-xs hover:bg-bg-700 ${val === selectedModel ? 'bg-accent/20 text-accent' : ''} truncate`}
                        onClick={() => {
                          setSelectedModel(val);
                          setModelDropdownOpen(false);
                          setModelSearch("");
                        }}
                      >
                        {m.name}
                      </button>
                    );
                  })}
                  {models.filter(m => m.name.toLowerCase().includes(modelSearch.toLowerCase()) || m.id.toLowerCase().includes(modelSearch.toLowerCase())).length === 0 && (
                    <div className="px-2 py-1 text-xs text-gray-500">No results</div>
                  )}
                </div>
              </div>
            )}
          </div>
          <select
            value={selectedContextId}
            onChange={(e) => setSelectedContextId(e.target.value)}
            className="bg-bg-700 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent w-36"
          >
            <option value="">No Context Profile</option>
            {contexts.map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        </div>
        <button onClick={() => cmd.hideOverlay().catch(() => {})}
          className="text-gray-500 hover:text-gray-300 p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>

      {/* ── Body (split view) ──────────────────────────────────────── */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Raw pane */}
        <div className="w-1/2 flex flex-col border-r border-bg-600">
          <div className="text-[10px] uppercase tracking-wider text-gray-500 px-2 py-1 border-b border-bg-600 shrink-0">
            Original
          </div>
          <textarea
            ref={rawRef}
            value={rawText}
            onChange={(e) => setRawText(e.target.value)}
            className="flex-1 bg-transparent p-2 text-sm resize-none focus:outline-none font-mono placeholder-gray-600"
            placeholder="Your raw prompt…"
          />
        </div>

        {/* Optimized pane */}
        <div className="w-1/2 flex flex-col">
          <div className="flex items-center justify-between text-[10px] uppercase tracking-wider text-gray-500 px-2 py-1 border-b border-bg-600 shrink-0">
            <span>Optimized</span>
            <div className="flex gap-2">
              {score != null && (
                <span className={`font-bold text-xs ${scoreColor}`}>
                  Score: {score}
                </span>
              )}
              {optimizedText.length > 0 && (
                <span className="text-gray-400">
                  {optimizedText.length} chars
                </span>
              )}
            </div>
          </div>
          {diffVisible ? (
            <pre className="flex-1 p-2 text-xs overflow-auto font-mono leading-relaxed whitespace-pre-wrap">
              {diff.split("\n").map((line, i) => {
                if (line.startsWith("+")) return <span key={i} className="diff-add">{line}{"\n"}</span>;
                if (line.startsWith("-")) return <span key={i} className="diff-del">{line}{"\n"}</span>;
                return <span key={i}>{line}{"\n"}</span>;
              })}
            </pre>
          ) : (
            <textarea
              ref={optRef}
              value={optimizedText}
              readOnly={isStreaming}
              className="flex-1 bg-transparent p-2 text-sm resize-none focus:outline-none font-mono"
              placeholder={isStreaming ? "Streaming…" : "Optimized text will appear here"}
              onChange={(e) => setOptimizedText(e.target.value)}
            />
          )}
        </div>
      </div>

      {/* ── Provider status banner (spec §11) ──────────────────────── */}
      {!providerAlive && (
        <div className="flex items-center gap-2 px-3 py-2 bg-yellow-900/30 border-t border-yellow-800/40 text-yellow-300 text-xs shrink-0">
          <AlertTriangle size={14} />
          <span className="flex-1">{activeProviderId || "Provider"} not reachable — optimization will fail.</span>
        </div>
      )}

      {/* ── Error banner ────────────────────────────────────────────── */}
      {error && (
        <div className="flex items-center gap-2 px-3 py-2 bg-red-900/30 border-t border-red-800/40 text-red-300 text-xs shrink-0">
          <AlertTriangle size={14} />
          <span className="flex-1 truncate">{error}</span>
          <button onClick={() => setError(null)} className="hover:text-white"><X size={12} /></button>
        </div>
      )}

      {/* ── Refinement Input Row ────────────────────────────────────── */}
      {refineActive && (
        <div className="flex items-center gap-2 px-3 py-2 bg-bg-800 border-t border-bg-600 shrink-0">
          <input
            ref={refineInputRef}
            type="text"
            value={refineNotes}
            onChange={(e) => setRefineNotes(e.target.value)}
            onKeyDown={handleRefineKeyDown}
            placeholder="Ask to refine (e.g., 'make it formal', 'add Python code')..."
            className="flex-1 bg-bg-950 border border-bg-600 rounded px-2.5 py-1.5 text-xs text-gray-200 focus:outline-none focus:border-accent font-sans"
          />
          <button
            onClick={handleSubmitRefine}
            disabled={isStreaming || !refineNotes.trim()}
            className="px-3 py-1.5 bg-accent hover:bg-accent/80 text-white rounded text-xs font-medium transition"
          >
            Submit
          </button>
        </div>
      )}

      {/* ── Footer (actions) ───────────────────────────────────────── */}
      <div className="flex items-center justify-between px-3 py-2 border-t border-bg-600 gap-2 shrink-0">
        <div className="flex gap-1.5">
          <button
            onClick={() => handleOptimize()}
            disabled={isStreaming || !rawText.trim()}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded bg-accent hover:bg-accent/80 disabled:opacity-40 disabled:cursor-not-allowed text-white transition"
          >
            {isStreaming ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
            {isStreaming ? "Optimizing…" : "Optimize"}
          </button>
          {optimizedText.trim() && (
            <button
              onClick={() => setRefineActive((a) => !a)}
              disabled={isStreaming}
              className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded border transition ${
                refineActive
                  ? "bg-bg-600 border-accent text-accent hover:bg-bg-500"
                  : "bg-bg-700 border-bg-600 text-gray-200 hover:bg-bg-600"
              }`}
              title="Refine optimized prompt (Ctrl+R)"
            >
              Refine
            </button>
          )}
          <button
            onClick={handleAccept}
            disabled={!optimizedText.trim() || isStreaming}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded bg-green-600 hover:bg-green-500 disabled:opacity-40 disabled:cursor-not-allowed text-white transition"
          >
            <Check size={13} /> Accept
          </button>
        </div>
        <div className="flex gap-1.5">
          {optimizedText && (
            <>
              <button
                onClick={() => setDiffVisible((v) => !v)}
                className="p-1.5 text-gray-400 hover:text-gray-200 rounded hover:bg-bg-600 transition"
                title="Toggle diff view"
              >
                <GitCompare size={14} />
              </button>
              <button
                onClick={handleSave}
                className="p-1.5 text-gray-400 hover:text-gray-200 rounded hover:bg-bg-600 transition"
                title="Save to library"
              >
                <BookOpen size={14} />
              </button>
              <button
                onClick={() => navigator.clipboard.writeText(optimizedText)}
                className="p-1.5 text-gray-400 hover:text-gray-200 rounded hover:bg-bg-600 transition"
                title="Copy to clipboard"
              >
                <Copy size={14} />
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
