import { useCallback, useEffect, useState } from "react";

import { api, toAppError } from "../ipc/client";
import type { ModelTier, SelfHostedModel } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";
import { Button, Chip, Panel, formatNumber } from "./primitives";

const TIERS: { value: ModelTier; label: string; hint: string }[] = [
  { value: "fast", label: "Fast", hint: "small models: simple edits and questions" },
  { value: "balanced", label: "Balanced", hint: "mid-size: most coding and writing" },
  { value: "powerful", label: "Powerful", hint: "large: multi-file refactors, migrations" },
];

const EMPTY = {
  id: undefined as string | undefined,
  displayName: "",
  baseUrl: "",
  model: "",
  tier: "balanced" as ModelTier,
  contextCap: 32768,
  apiKey: "",
};

const inputClass =
  "mt-1 w-full rounded border border-ink-700 bg-ink-900 px-3 py-2 text-sm text-ink-100 select-text";

/**
 * Models the user runs on their own machines behind an OpenAI-compatible API
 * (Ollama, vLLM, LM Studio, llama.cpp server). The prompt router treats them
 * like any other model: they cost nothing per token, so they are preferred
 * whenever their tier is enough for the request.
 */
export function SelfHostedModelsPanel() {
  const { setError, setNotice } = useAppStore();
  const [models, setModels] = useState<SelfHostedModel[]>([]);
  const [form, setForm] = useState(EMPTY);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(true);
  const [removing, setRemoving] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [tests, setTests] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    try {
      setModels(await api.listSelfHostedModels());
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setLoading(false);
    }
  }, [setError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const saved = await api.saveSelfHostedModel({
        id: form.id,
        displayName: form.displayName,
        baseUrl: form.baseUrl,
        model: form.model,
        tier: form.tier,
        contextCap: form.contextCap,
        apiKey: form.apiKey.trim() ? form.apiKey : undefined,
      });
      setForm(EMPTY);
      setShowForm(false);
      setTests((current) => {
        const next = { ...current };
        delete next[saved.id];
        return next;
      });
      setNotice(`Saved ${saved.displayName}. Use Check connection to make sure it responds.`);
      await refresh();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setSaving(false);
    }
  };

  const test = async (model: SelfHostedModel) => {
    setTesting(model.id);
    try {
      const result = await api.testSelfHostedModel(model.id);
      setTests((t) => ({
        ...t,
        [model.id]: `OK · ${formatNumber(result.latencyMs)} ms · "${result.reply}"`,
      }));
    } catch (err) {
      setTests((t) => ({ ...t, [model.id]: `Failed: ${toAppError(err).message}` }));
    } finally {
      setTesting(null);
    }
  };

  const remove = async (model: SelfHostedModel) => {
    setRemoving(model.id);
    try {
      await api.deleteSelfHostedModel(model.id);
      if (form.id === model.id) {
        setForm(EMPTY);
        setShowForm(false);
      }
      setNotice(`Removed ${model.displayName}.`);
      await refresh();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setRemoving(null);
    }
  };

  return (
    <Panel
      title="Your own server"
      description="Connect a model from Ollama, LM Studio, or another OpenAI-compatible service."
      actions={
        <Button
          disabled={showForm || loading}
          onClick={() => {
            setForm(EMPTY);
            setShowForm(true);
          }}
        >
          Add model
        </Button>
      }
    >
      <div className="space-y-4">
        {loading ? (
          <p role="status" className="text-sm text-ink-400">
            Loading models…
          </p>
        ) : models.length === 0 ? (
          <div className="py-5 text-center">
            <p className="text-sm font-medium text-ink-200">Connect your first model</p>
            <p className="mt-1 text-xs text-ink-400">
              Start your model server, then add its address and model name here.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-ink-800">
            {models.map((m) => (
              <div key={m.id} className="flex flex-wrap items-center justify-between gap-3 py-2.5">
                <div className="min-w-0 space-y-0.5">
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-medium text-ink-100">{m.displayName}</span>
                    <Chip tone="neutral">{m.tier}</Chip>
                    {m.hasApiKey ? <Chip tone="neutral">Key saved</Chip> : null}
                  </div>
                  <p className="mono truncate text-[11px] text-ink-500">
                    {m.model} @ {m.baseUrl} · {formatNumber(m.contextCap)} token context
                  </p>
                  {tests[m.id] ? (
                    <p
                      role="status"
                      className={`text-[11px] ${tests[m.id]?.startsWith("OK") ? "text-ok" : "text-danger"}`}
                    >
                      {tests[m.id]}
                    </p>
                  ) : null}
                </div>
                <div className="flex gap-2">
                  <Button
                    onClick={() => void test(m)}
                    disabled={testing !== null || removing !== null || saving}
                  >
                    {testing === m.id ? "Checking…" : "Check connection"}
                  </Button>
                  <Button
                    variant="ghost"
                    disabled={saving || removing !== null || testing !== null}
                    onClick={() => {
                      setShowForm(true);
                      setForm({
                        id: m.id,
                        displayName: m.displayName,
                        baseUrl: m.baseUrl,
                        model: m.model,
                        tier: m.tier,
                        contextCap: m.contextCap,
                        apiKey: "",
                      });
                    }}
                  >
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    disabled={removing !== null || saving || testing !== null}
                    onClick={() => void remove(m)}
                  >
                    {removing === m.id ? "Removing…" : "Remove"}
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}

        {showForm && (
          <form
            onSubmit={save}
            className="space-y-3 rounded border border-ink-800 bg-ink-950/60 p-3"
          >
            <h4 className="text-xs font-medium text-ink-200">
              {form.id ? `Edit ${form.displayName}` : "Connect a model"}
            </h4>
            <fieldset disabled={saving} className="space-y-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="block text-[11px] text-ink-400">
                  Display name
                  <input
                    className={inputClass}
                    autoFocus
                    placeholder="e.g. My coding model"
                    value={form.displayName}
                    onChange={(e) => setForm({ ...form, displayName: e.target.value })}
                    required
                  />
                </label>
                <label className="block text-[11px] text-ink-400">
                  Server address
                  <input
                    className={inputClass}
                    type="url"
                    placeholder="http://localhost:11434"
                    value={form.baseUrl}
                    onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
                    required
                  />
                </label>
                <label className="block text-[11px] text-ink-400">
                  Model name on the server
                  <input
                    className={inputClass}
                    placeholder="e.g. qwen3:14b or gemma3:12b"
                    value={form.model}
                    onChange={(e) => setForm({ ...form, model: e.target.value })}
                    required
                  />
                </label>
                <label className="block text-[11px] text-ink-400">
                  API key (optional)
                  <input
                    className={inputClass}
                    type="password"
                    autoComplete="off"
                    placeholder={
                      form.id
                        ? "Leave blank to keep the current key"
                        : "Only if your server needs one"
                    }
                    value={form.apiKey}
                    onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
                  />
                </label>
              </div>
              <details className="text-xs text-ink-400">
                <summary className="cursor-pointer">
                  Model options · {TIERS.find((tier) => tier.value === form.tier)?.label} ·{" "}
                  {formatNumber(form.contextCap)} tokens
                </summary>
                <p className="mt-2">
                  Choose a tier and context size that match your model. These settings help LeanAI
                  choose it for the right tasks.
                </p>
                <div className="mt-3 grid gap-3 sm:grid-cols-2">
                  <label className="block text-[11px] text-ink-400">
                    Capability tier
                    <select
                      className={inputClass}
                      value={form.tier}
                      onChange={(e) => setForm({ ...form, tier: e.target.value as ModelTier })}
                    >
                      {TIERS.map((t) => (
                        <option key={t.value} value={t.value}>
                          {t.label} ({t.hint})
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block text-[11px] text-ink-400">
                    Context window (tokens)
                    <input
                      className={inputClass}
                      type="number"
                      min={2048}
                      max={2000000}
                      required
                      value={form.contextCap || ""}
                      onChange={(e) => setForm({ ...form, contextCap: Number(e.target.value) })}
                    />
                  </label>
                </div>
              </details>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-[11px] text-ink-500">
                  Use the server’s address with or without <code>/v1</code>. API keys are stored
                  securely. These models are preferred when suitable; your hosting service may still
                  charge for use.
                </p>
                <div className="flex shrink-0 gap-2">
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setForm(EMPTY);
                      setShowForm(false);
                    }}
                  >
                    Cancel
                  </Button>
                  <Button
                    type="submit"
                    variant="primary"
                    disabled={
                      saving ||
                      !form.displayName.trim() ||
                      !form.baseUrl.trim() ||
                      !form.model.trim()
                    }
                  >
                    {saving ? "Saving…" : form.id ? "Save changes" : "Add model"}
                  </Button>
                </div>
              </div>
            </fieldset>
          </form>
        )}
      </div>
    </Panel>
  );
}
