/** Overlay window root — renders the PromptOpt overlay UI. */
import { useEffect, useState, useRef, useCallback } from "react";
import {
  cmd, onOptChunk, onOptDone, onOptError,
  type FrameworkInfo, type ModelInfo, type Settings,
} from "../lib/tauri";
import {
  Sparkles, Check, Copy, RotateCcw,
  Loader2, AlertTriangle, X, BookOpen, Zap,
} from "lucide-react";

export default function OverlayApp() {
  // ── State ────────────────────────────────────────────────────────────
  const [rawText, setRawText] = useState("");
  const [optimizedText, setOptimizedText] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [score, setScore] = useState<number | null>(null);
  const [diff, setDiff] = useState("");
  const [diffVisible, setDiffVisible] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [frameworks, setFrameworks] = useState<FrameworkInfo[]>([]);
  const [selectedFramework, setSelectedFramework] = useState("CREATE");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("ollama:llama3");
  const [_settings, setSettings] = useState<Settings | null>(null);

  const sessionIdRef = useRef("");
  const rawRef = useRef<HTMLTextAreaElement>(null);
  const optRef = useRef<HTMLTextAreaElement>(null);

  // ── Init ─────────────────────────────────────────────────────────────
  useEffect(() => {
    (async () => {
      try {
        const [fw, st] = await Promise.all([cmd.listFrameworks(), cmd.getSettings()]);
        setFrameworks(fw);
        setSettings(st);
        setSelectedFramework(st.default_framework);
        setSelectedModel(st.default_model);

        // Load models for ollama.
        try {
          const ms = await cmd.getModels("ollama");
          setModels(ms);
          if (ms.length > 0 && st.default_model === "ollama:llama3") {
            setSelectedModel(`ollama:${ms[0].id}`);
          }
        } catch {
          // Ollama not running — models stay empty.
        }

        // Capture text from active field.
        try {
          const cap = await cmd.captureText();
          if (cap.text) setRawText(cap.text);
        } catch {
          // Accessibility may not be available.
        }
      } catch (e: any) {
        console.error("init error", e);
      }
    })();

    // Listen for streaming events.
    type UnlistenFn = () => void;
    const cleaners: UnlistenFn[] = [];
    const setup = async () => {
      cleaners.push(await onOptChunk((ev) => {
        if (ev.session_id === sessionIdRef.current) {
          setOptimizedText((prev) => prev + ev.text);
        }
      }));
      cleaners.push(await onOptDone((ev) => {
        if (ev.session_id === sessionIdRef.current) {
          setScore(ev.score);
          setDiff(ev.diff);
          setIsStreaming(false);
        }
      }));
      cleaners.push(await onOptError((ev) => {
        if (ev.session_id === sessionIdRef.current) {
          setError(ev.message);
          setIsStreaming(false);
        }
      }));
    };
    setup();

    return () => { cleaners.forEach((fn) => fn()); };
  }, []);

  // ── Actions ───────────────────────────────────────────────────────────
  const handleOptimize = useCallback(async () => {
    if (!rawText.trim()) return;
    sessionIdRef.current = crypto.randomUUID();
    setOptimizedText("");
    setScore(null);
    setDiff("");
    setError(null);
    setIsStreaming(true);

    try {
      await cmd.optimizePrompt({
        raw: rawText,
        framework: selectedFramework,
        model: selectedModel,
      });
    } catch (e: any) {
      setError(String(e));
      setIsStreaming(false);
    }
  }, [rawText, selectedFramework, selectedModel]);

  const handleAccept = useCallback(async () => {
    if (!optimizedText.trim()) return;
    try {
      await cmd.acceptReplacement(optimizedText);
    } catch (e: any) {
      // Fallback: copy to clipboard and let user paste.
      try {
        await navigator.clipboard.writeText(optimizedText);
        alert("Replaced via clipboard fallback. Press Ctrl+V to paste.");
      } catch {
        alert("Replacement failed. Text copied — paste manually.");
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

  // ── Keyboard shortcuts inside overlay ─────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        cmd.hideOverlay().catch(() => {});
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // ── Score color ───────────────────────────────────────────────────────
  const scoreColor = score == null ? "text-gray-500"
    : score >= 70 ? "text-green-400"
    : score >= 40 ? "text-yellow-400"
    : "text-red-400";

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div className="flex flex-col h-screen bg-bg-900 text-gray-200 select-none"
      data-tauri-drag-region
    >
      {/* ── Header ─────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-bg-600 gap-2 shrink-0">
        <div className="flex items-center gap-2 text-sm">
          <Zap size={14} className="text-accent" />
          <select
            value={selectedFramework}
            onChange={(e) => setSelectedFramework(e.target.value)}
            className="bg-bg-700 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent"
          >
            {frameworks.map((f) => (
              <option key={f.id} value={f.id}>{f.name}</option>
            ))}
          </select>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            className="bg-bg-700 text-xs px-2 py-1 rounded border border-bg-600 focus:outline-none focus:border-accent w-40"
          >
            <option value="ollama:llama3">ollama:llama3</option>
            {models.map((m) => (
              <option key={m.id} value={`ollama:${m.id}`}>{m.name}</option>
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
            {score != null && (
              <span className={`font-bold text-xs ${scoreColor}`}>
                {score}/100
              </span>
            )}
          </div>
          {diffVisible ? (
            <pre className="flex-1 p-2 text-xs overflow-auto font-mono leading-relaxed whitespace-pre-wrap">
              {diff.split("\n").map((line, i) => {
                if (line.startsWith("+")) return <span key={i} className="diff-add">{line}\n</span>;
                if (line.startsWith("-")) return <span key={i} className="diff-del">{line}\n</span>;
                return <span key={i}>{line}\n</span>;
              })}
            </pre>
          ) : (
            <textarea
              ref={optRef}
              value={isStreaming ? undefined : optimizedText}
              readOnly={isStreaming}
              className="flex-1 bg-transparent p-2 text-sm resize-none focus:outline-none font-mono"
              placeholder={isStreaming ? "Streaming…" : "Optimized text will appear here"}
              onChange={(e) => setOptimizedText(e.target.value)}
            />
          )}
        </div>
      </div>

      {/* ── Error banner ────────────────────────────────────────────── */}
      {error && (
        <div className="flex items-center gap-2 px-3 py-2 bg-red-900/30 border-t border-red-800/40 text-red-300 text-xs shrink-0">
          <AlertTriangle size={14} />
          <span className="flex-1 truncate">{error}</span>
          <button onClick={() => setError(null)} className="hover:text-white"><X size={12} /></button>
        </div>
      )}

      {/* ── Footer (actions) ───────────────────────────────────────── */}
      <div className="flex items-center justify-between px-3 py-2 border-t border-bg-600 gap-2 shrink-0">
        <div className="flex gap-1.5">
          <button
            onClick={handleOptimize}
            disabled={isStreaming || !rawText.trim()}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded bg-accent hover:bg-accent/80 disabled:opacity-40 disabled:cursor-not-allowed text-white transition"
          >
            {isStreaming ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
            {isStreaming ? "Optimizing…" : "Optimize"}
          </button>
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
                <RotateCcw size={14} />
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
