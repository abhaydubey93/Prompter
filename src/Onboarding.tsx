import { useEffect, useState } from "react";
import {
  CheckCircle2,
  Loader2,
  ChevronRight,
  ChevronLeft,
  AlertTriangle,
  X,
} from "lucide-react";
import { cmd, ProviderConfig, ModelInfo } from "./lib/tauri";

interface Props {
  onClose: () => void;
}

const KIND_LABEL: Record<string, string> = {
  ollama: "Ollama (local)",
  openai_compat: "OpenAI-compatible",
  anthropic: "Anthropic",
  gemini: "Google Gemini",
};

export default function Onboarding({ onClose }: Props) {
  const [step, setStep] = useState(0);
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [selected, setSelected] = useState<ProviderConfig | null>(null);
  const [endpoint, setEndpoint] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [needsKey, setNeedsKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ alive: boolean; models: ModelInfo[]; error: string | null } | null>(null);
  const [chosenModel, setChosenModel] = useState<string>("");

  useEffect(() => {
    cmd.listProviders().then((ps) => setProviders(ps)).catch(() => {});
  }, []);

  function pickProvider(p: ProviderConfig) {
    setSelected(p);
    setEndpoint(p.base_url);
    setApiKey("");
    setNeedsKey(!!p.api_key_slot);
    setTestResult(null);
    setChosenModel(p.default_model || "");
    setStep(1);
  }

  async function testConn() {
    if (!selected) return;
    setTesting(true);
    setTestResult(null);
    try {
      // Save endpoint + key first so test_provider picks them up.
      await cmd.saveProvider({ ...selected, base_url: endpoint });
      if (needsKey && apiKey) {
        await cmd.setProviderKey(selected.id, apiKey);
        await cmd.setProviderEnabled(selected.id, true);
      } else if (!needsKey) {
        await cmd.setProviderEnabled(selected.id, true);
      }
      const r = await cmd.testProvider(selected.id);
      setTestResult(r);
      if (r.alive && r.models.length && !chosenModel) {
        setChosenModel(`${selected.id}:${r.models[0].id}`);
      }
    } catch (e) {
      setTestResult({ alive: false, models: [], error: String(e) });
    } finally {
      setTesting(false);
    }
  }

  async function finish(skipped: boolean) {
    try {
      await cmd.completeOnboarding(
        skipped ? null : selected?.id ?? null,
        skipped ? null : chosenModel || null,
        skipped
      );
    } catch (e) {
      console.error("complete_onboarding failed", e);
    }
    onClose();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-2xl rounded-2xl border border-zinc-800 bg-zinc-900 shadow-2xl">
        <div className="flex items-center justify-between border-b border-zinc-800 px-6 py-4">
          <h2 className="text-lg font-semibold text-zinc-100">
            Welcome to Prompter Overlay
          </h2>
          <button
            onClick={() => finish(true)}
            className="text-zinc-500 hover:text-zinc-200"
            title="Skip setup"
          >
            <X size={18} />
          </button>
        </div>

        {/* Progress dots */}
        <div className="flex gap-2 px-6 pt-4">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className={`h-1.5 flex-1 rounded-full ${i <= step ? "bg-emerald-500" : "bg-zinc-700"}`}
            />
          ))}
        </div>

        <div className="p-6">
          {/* STEP 0: provider picker */}
          {step === 0 && (
            <div>
              <p className="mb-4 text-sm text-zinc-400">
                Choose your LLM provider. You can add more later in Settings.
              </p>
              <div className="grid grid-cols-2 gap-3">
                {providers.map((p) => (
                  <button
                    key={p.id}
                    onClick={() => pickProvider(p)}
                    className={`rounded-lg border p-4 text-left transition ${
                      p.enabled
                        ? "border-emerald-700 bg-emerald-950/30 hover:border-emerald-600"
                        : "border-zinc-800 bg-zinc-950 hover:border-zinc-700"
                    }`}
                  >
                    <div className="font-medium text-zinc-100">{p.label}</div>
                    <div className="text-xs text-zinc-500">{KIND_LABEL[p.kind] ?? p.kind}</div>
                    <div className="mt-1 truncate text-xs text-zinc-600">{p.base_url}</div>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* STEP 1: configure provider */}
          {step === 1 && selected && (
            <div>
              <button
                onClick={() => setStep(0)}
                className="mb-3 flex items-center gap-1 text-xs text-zinc-500 hover:text-zinc-300"
              >
                <ChevronLeft size={14} /> Back
              </button>
              <h3 className="mb-1 font-medium text-zinc-100">{selected.label}</h3>
              <p className="mb-4 text-xs text-zinc-500">{KIND_LABEL[selected.kind]}</p>

              <label className="mb-1 block text-xs text-zinc-400">Endpoint</label>
              <input
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                className="mb-4 w-full rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
              />

              {needsKey && (
                <>
                  <label className="mb-1 block text-xs text-zinc-400">API Key (stored in OS keychain)</label>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-..."
                    className="mb-4 w-full rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                  />
                </>
              )}

              <button
                onClick={testConn}
                disabled={testing || (needsKey && !apiKey)}
                className="flex items-center gap-2 rounded-md bg-zinc-800 px-4 py-2 text-sm text-zinc-100 hover:bg-zinc-700 disabled:opacity-40"
              >
                {testing && <Loader2 size={14} className="animate-spin" />}
                Test connection
              </button>

              {testResult && (
                <div className={`mt-4 rounded-md border p-3 text-sm ${
                  testResult.alive
                    ? "border-emerald-800 bg-emerald-950/30 text-emerald-300"
                    : "border-red-900 bg-red-950/30 text-red-300"
                }`}>
                  {testResult.alive ? (
                    <div className="flex items-center gap-2">
                      <CheckCircle2 size={16} /> Connected — {testResult.models.length} model(s) found
                    </div>
                  ) : (
                    <div className="flex items-center gap-2">
                      <AlertTriangle size={16} /> Failed: {testResult.error || "unreachable"}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* STEP 2: pick model + finish */}
          {step === 2 && (
            <div>
              <h3 className="mb-3 font-medium text-zinc-100">Default model</h3>
              {testResult?.alive && testResult.models.length > 0 ? (
                <select
                  value={chosenModel}
                  onChange={(e) => setChosenModel(e.target.value)}
                  className="mb-4 w-full rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                >
                  {testResult.models.map((m) => (
                    <option key={m.id} value={`${selected?.id}:${m.id}`}>
                      {m.name}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  value={chosenModel}
                  onChange={(e) => setChosenModel(e.target.value)}
                  placeholder={`${selected?.id}:model-name`}
                  className="mb-4 w-full rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                />
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-zinc-800 px-6 py-4">
          <button
            onClick={() => finish(true)}
            className="text-sm text-zinc-500 hover:text-zinc-300"
          >
            Skip
          </button>
          {step === 1 && testResult?.alive && (
            <button
              onClick={() => setStep(2)}
              className="flex items-center gap-1 rounded-md bg-emerald-600 px-4 py-2 text-sm text-white hover:bg-emerald-500"
            >
              Next <ChevronRight size={14} />
            </button>
          )}
          {step === 2 && (
            <button
              onClick={() => finish(false)}
              className="flex items-center gap-1 rounded-md bg-emerald-600 px-4 py-2 text-sm text-white hover:bg-emerald-500"
            >
              <CheckCircle2 size={14} /> Finish
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
