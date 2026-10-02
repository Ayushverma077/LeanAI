import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";

import { Button, Chip, Field, Panel, Toggle, formatNumber } from "../components/primitives";
import { SelfHostedModelsPanel } from "../components/SelfHostedModelsPanel";
import { api, toAppError } from "../ipc/client";
import type {
  ModelRecord,
  PriceCatalog,
  ProviderConfig,
  RoutingDecision,
  SidecarStatus,
  TaskClass,
  TokenCountResult,
} from "../ipc/types";
import { useAppStore } from "../store/useAppStore";

export function ModelsPage() {
  const { setError, setNotice } = useAppStore();

  const [models, setModels] = useState<ModelRecord[]>([]);
  const [sidecarStatus, setSidecarStatus] = useState<SidecarStatus>({ state: "stopped" });
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [catalog, setCatalog] = useState<PriceCatalog | null>(null);

  const [section, setSection] = useState<"cloud" | "server" | "local">("cloud");
  const [showLocalForm, setShowLocalForm] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<string | null>(null);

  // Register model form state
  const [newModelName, setNewModelName] = useState("");
  const [newModelPath, setNewModelPath] = useState("");
  const [newModelContext, setNewModelContext] = useState(8192);
  const [registering, setRegistering] = useState(false);

  // Provider configuration state
  const [configuringProvider, setConfiguringProvider] = useState<string | null>(null);
  const [credentialSecret, setCredentialSecret] = useState("");

  // Routing test state
  const [taskClass, setTaskClass] = useState<TaskClass>("multi_file_planning");
  const [estimatedTokens, setEstimatedTokens] = useState(16000);
  const [budgetUsd, setBudgetUsd] = useState<string>("");
  const [requireLocalOnly, setRequireLocalOnly] = useState(false);
  const [routingResult, setRoutingResult] = useState<RoutingDecision | null>(null);

  // Exact token counting test state
  const [sampleText, setSampleText] = useState("LeanAI offline repository context bundler.");
  const [exactCountOptIn, setExactCountOptIn] = useState(false);
  const [tokenResult, setTokenResult] = useState<TokenCountResult | null>(null);

  const refreshData = useCallback(async () => {
    try {
      const [m, status, p, c] = await Promise.all([
        api.listModels(),
        api.localModelStatus(),
        api.listProviders(),
        api.getModelCatalog(),
      ]);
      setModels(m);
      setSidecarStatus(status);
      setProviders(p);
      setCatalog(c);
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setLoading(false);
    }
  }, [setError]);

  useEffect(() => {
    void refreshData();
  }, [refreshData]);

  const handleRegisterModel = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newModelName.trim() || !newModelPath.trim()) return;
    setRegistering(true);
    try {
      await api.registerLocalModel(newModelName.trim(), newModelPath.trim(), newModelContext);
      setNewModelName("");
      setNewModelPath("");
      setShowLocalForm(false);
      setNotice(`Added local model: ${newModelName}`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setRegistering(false);
    }
  };

  const handleUnregisterModel = async (id: string, name: string) => {
    setPending("local");
    try {
      await api.unregisterModel(id);
      setNotice(`Removed model: ${name}`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setPending(null);
    }
  };

  const handleStartSidecar = async (id: string) => {
    setPending("local");
    try {
      const status = await api.startLocalModel(id);
      setSidecarStatus(status);
      setNotice("Local model started.");
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setPending(null);
    }
  };

  const handleStopSidecar = async () => {
    setPending("local");
    try {
      const status = await api.stopLocalModel();
      setSidecarStatus(status);
      setNotice("Local model stopped.");
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setPending(null);
    }
  };

  const handleConfigureCredential = async (providerId: string) => {
    if (!credentialSecret.trim()) return;
    setPending(providerId);
    try {
      await api.configureProviderCredential(providerId, "api_key", credentialSecret.trim());
      setConfiguringProvider(null);
      setCredentialSecret("");
      setNotice(`Credential securely stored in OS keychain for ${providerId}.`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setPending(null);
    }
  };

  const handleDisconnectProvider = async (providerId: string) => {
    setPending(providerId);
    try {
      await api.disconnectProvider(providerId);
      setNotice(`Disconnected ${providerId} and removed key from OS keychain.`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setPending(null);
    }
  };

  const handleEvaluateRoute = async () => {
    try {
      const budget = budgetUsd ? parseFloat(budgetUsd) : undefined;
      const decision = await api.routeTask(taskClass, estimatedTokens, budget, requireLocalOnly);
      setRoutingResult(decision);
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const handleEstimateTokens = async () => {
    try {
      const result = await api.estimateProviderTokens(
        sampleText,
        "openai",
        "gpt-4o",
        exactCountOptIn,
      );
      setTokenResult(result);
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const browseModel = async () => {
    try {
      const selected = await open({
        title: "Choose a model file",
        multiple: false,
        filters: [{ name: "GGUF model", extensions: ["gguf"] }],
      });
      if (typeof selected === "string") {
        setNewModelPath(selected);
        if (!newModelName.trim()) {
          setNewModelName(
            selected
              .split(/[\\/]/)
              .pop()
              ?.replace(/\.gguf$/i, "") ?? "",
          );
        }
      }
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const localBusy =
    pending === "local" || sidecarStatus.state === "starting" || sidecarStatus.state === "stopping";

  return (
    <div className="mx-auto max-w-5xl space-y-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-lg font-semibold text-ink-100">Models</h1>
          <p className="mt-1 text-sm text-ink-400">
            Choose where your AI runs. Connect a provider or add a model you already have.
          </p>
        </div>
        <Button
          variant="ghost"
          onClick={() => {
            setLoading(true);
            void refreshData();
          }}
          disabled={loading || pending !== null}
        >
          {loading ? "Loading…" : "Refresh"}
        </Button>
      </div>

      <div className="grid gap-3 sm:grid-cols-3" role="group" aria-label="Model source">
        {(
          [
            { id: "cloud", title: "Cloud provider", hint: "Connect with an API key" },
            { id: "server", title: "Your own server", hint: "Ollama, LM Studio or a custom API" },
            { id: "local", title: "Local model file", hint: "Run a GGUF file on this computer" },
          ] as const
        ).map((item) => (
          <button
            key={item.id}
            type="button"
            aria-pressed={section === item.id}
            onClick={() => {
              setSection(item.id);
              setConfiguringProvider(null);
              setCredentialSecret("");
            }}
            className={`rounded-lg border p-4 text-left transition-colors ${section === item.id ? "border-brand bg-brand/10" : "border-ink-800 bg-ink-900/60 hover:border-ink-700"}`}
          >
            <span className="block text-sm font-semibold text-ink-100">{item.title}</span>
            <span className="mt-1 block text-xs text-ink-400">{item.hint}</span>
          </button>
        ))}
      </div>

      {section === "cloud" && (
        <Panel
          title="Cloud providers"
          description="Add your provider’s API key. Keys are saved securely in your system keychain."
        >
          {loading ? (
            <p role="status" className="text-sm text-ink-400">
              Loading providers…
            </p>
          ) : providers.length === 0 ? (
            <p className="text-sm text-ink-400">No providers available. Try refreshing.</p>
          ) : (
            <div className="divide-y divide-ink-800">
              {providers.map((provider) => (
                <div key={provider.providerId} className="py-3 first:pt-0 last:pb-0">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="flex flex-wrap items-center gap-3">
                      <span className="text-sm font-medium text-ink-100">
                        {provider.displayName}
                      </span>
                      <Chip tone={provider.isConfigured ? "ok" : "neutral"}>
                        {provider.isConfigured ? "Key saved" : "Not connected"}
                      </Chip>
                    </div>
                    {provider.isConfigured ? (
                      <Button
                        variant="ghost"
                        disabled={pending !== null}
                        onClick={() => void handleDisconnectProvider(provider.providerId)}
                      >
                        {pending === provider.providerId ? "Disconnecting…" : "Disconnect"}
                      </Button>
                    ) : configuringProvider !== provider.providerId ? (
                      <Button
                        disabled={pending !== null}
                        onClick={() => {
                          setConfiguringProvider(provider.providerId);
                          setCredentialSecret("");
                        }}
                      >
                        Connect
                      </Button>
                    ) : null}
                  </div>
                  {configuringProvider === provider.providerId && (
                    <form
                      onSubmit={(event) => {
                        event.preventDefault();
                        void handleConfigureCredential(provider.providerId);
                      }}
                      className="mt-3 space-y-3 rounded-lg border border-ink-800 bg-ink-950/60 p-4"
                    >
                      <Field label={`${provider.displayName} API key`}>
                        <input
                          autoFocus
                          type="password"
                          autoComplete="off"
                          required
                          value={credentialSecret}
                          onChange={(event) => setCredentialSecret(event.target.value)}
                          className="w-full rounded-md border border-ink-700 bg-ink-900 px-3 py-2 text-sm text-ink-100"
                          placeholder="Paste your API key"
                          disabled={pending !== null}
                        />
                      </Field>
                      <div className="flex justify-end gap-2">
                        <Button
                          variant="ghost"
                          disabled={pending !== null}
                          onClick={() => {
                            setConfiguringProvider(null);
                            setCredentialSecret("");
                          }}
                        >
                          Cancel
                        </Button>
                        <Button
                          type="submit"
                          variant="primary"
                          disabled={pending !== null || !credentialSecret.trim()}
                        >
                          {pending === provider.providerId ? "Saving…" : "Save key"}
                        </Button>
                      </div>
                    </form>
                  )}
                </div>
              ))}
            </div>
          )}
        </Panel>
      )}

      <div hidden={section !== "server"}>
        <SelfHostedModelsPanel />
      </div>

      {section === "local" && (
        <Panel
          title="Local models"
          description="Add a GGUF file, then start it here. Requires llama-server installed on this computer."
          actions={
            <Button onClick={() => setShowLocalForm(true)} disabled={showLocalForm}>
              Add model
            </Button>
          }
        >
          <div className="space-y-4">
            <div
              role="status"
              className="flex flex-wrap items-center justify-between gap-3 rounded-lg bg-ink-950/60 p-3"
            >
              <span className="text-sm text-ink-300">
                {sidecarStatus.state === "ready"
                  ? `Running: ${sidecarStatus.displayName}`
                  : sidecarStatus.state === "error"
                    ? sidecarStatus.message
                    : sidecarStatus.state === "starting"
                      ? "Starting model…"
                      : sidecarStatus.state === "stopping"
                        ? "Stopping model…"
                        : "No model running"}
              </span>
              {sidecarStatus.state === "ready" && (
                <Button disabled={localBusy} onClick={handleStopSidecar}>
                  {localBusy ? "Stopping…" : "Stop model"}
                </Button>
              )}
            </div>
            {loading ? (
              <p className="text-sm text-ink-400">Loading models…</p>
            ) : models.length === 0 ? (
              <div className="py-5 text-center">
                <p className="text-sm font-medium text-ink-200">Add your first local model</p>
                <p className="mt-1 text-xs text-ink-400">
                  Choose a .gguf file from your computer to get started.
                </p>
              </div>
            ) : (
              <div className="divide-y divide-ink-800">
                {models.map((model) => {
                  const running =
                    sidecarStatus.state === "ready" && sidecarStatus.modelId === model.id;
                  return (
                    <div
                      key={model.id}
                      className="flex flex-wrap items-center justify-between gap-3 py-3"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-ink-100">{model.displayName}</p>
                        <p className="mt-1 text-xs text-ink-400">
                          {formatNumber(model.capabilityProfile.contextCap)} token context
                        </p>
                        <p
                          className="mt-1 truncate text-xs text-ink-500"
                          title={model.filePath ?? ""}
                        >
                          {model.filePath}
                        </p>
                      </div>
                      <div className="flex gap-2">
                        <Button
                          variant="primary"
                          disabled={running || localBusy}
                          onClick={() => void handleStartSidecar(model.id)}
                        >
                          {running ? "Running" : "Start"}
                        </Button>
                        <Button
                          variant="ghost"
                          disabled={localBusy}
                          onClick={() => void handleUnregisterModel(model.id, model.displayName)}
                        >
                          Remove
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
            {showLocalForm && (
              <form
                onSubmit={handleRegisterModel}
                className="space-y-4 rounded-lg border border-ink-800 bg-ink-950/60 p-4"
              >
                <h3 className="text-sm font-medium text-ink-100">Add a local model</h3>
                <fieldset disabled={registering} className="space-y-4">
                  <div className="flex items-end gap-2">
                    <div className="min-w-0 flex-1">
                      <Field label="Model file">
                        <input
                          autoFocus
                          required
                          value={newModelPath}
                          onChange={(event) => setNewModelPath(event.target.value)}
                          placeholder="Choose a .gguf file or paste its path"
                          className="w-full rounded-md border border-ink-700 bg-ink-900 px-3 py-2 text-sm text-ink-100"
                        />
                      </Field>
                    </div>
                    <Button onClick={browseModel}>Browse…</Button>
                  </div>
                  <Field label="Display name">
                    <input
                      required
                      value={newModelName}
                      onChange={(event) => setNewModelName(event.target.value)}
                      placeholder="A name you’ll recognize"
                      className="w-full rounded-md border border-ink-700 bg-ink-900 px-3 py-2 text-sm text-ink-100"
                    />
                  </Field>
                  <details className="text-xs text-ink-400">
                    <summary className="cursor-pointer">Model options</summary>
                    <div className="mt-3">
                      <Field
                        label="Context window (tokens)"
                        hint="Use a value supported by your model and computer."
                      >
                        <input
                          type="number"
                          min={1}
                          required
                          value={newModelContext || ""}
                          onChange={(event) => setNewModelContext(Number(event.target.value))}
                          className="w-full rounded-md border border-ink-700 bg-ink-900 px-3 py-2 text-sm"
                        />
                      </Field>
                    </div>
                  </details>
                  <div className="flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      onClick={() => {
                        setShowLocalForm(false);
                        setNewModelName("");
                        setNewModelPath("");
                        setNewModelContext(8192);
                      }}
                    >
                      Cancel
                    </Button>
                    <Button
                      type="submit"
                      variant="primary"
                      disabled={registering || !newModelName.trim() || !newModelPath.trim()}
                    >
                      {registering ? "Adding model…" : "Add model"}
                    </Button>
                  </div>
                </fieldset>
              </form>
            )}
          </div>
        </Panel>
      )}

      <div className="border-t border-ink-800 pt-4">
        <button
          type="button"
          aria-expanded={showAdvanced}
          aria-controls="model-advanced-tools"
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="rounded text-sm font-medium text-ink-300 hover:text-ink-100"
        >
          {showAdvanced ? "▾" : "▸"} Advanced tools
        </button>
        <p className="mt-1 text-xs text-ink-400">
          Compare prices, preview model selection, and count tokens.
        </p>
      </div>
      <div id="model-advanced-tools" hidden={!showAdvanced} className="space-y-4">
        {/* Section 3: Cost-Aware Routing & Exact Token Counting */}
        <div className="grid gap-4 lg:grid-cols-2">
          <Panel
            title="Preview model selection"
            description="See which model would handle a task and its estimated cost."
          >
            <div className="space-y-3">
              <Field label="Task Class" hint="Determines reasoning complexity requirement.">
                <select
                  value={taskClass}
                  onChange={(e) => setTaskClass(e.target.value as TaskClass)}
                  className="w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                >
                  <option value="file_inventory">File Inventory (Low Cost)</option>
                  <option value="context_documentation">Context Documentation (Low Cost)</option>
                  <option value="multi_file_planning">
                    Multi-File Planning (Strong Reasoning)
                  </option>
                  <option value="code_change_proposal">
                    Code Change Proposal (Strong Reasoning)
                  </option>
                  <option value="mechanical_validation">Mechanical Validation (Low Cost)</option>
                </select>
              </Field>

              <Field
                label="Estimated Context Tokens"
                hint="Escalates if size exceeds low-cost tier context cap."
              >
                <input
                  type="number"
                  value={estimatedTokens}
                  onChange={(e) => setEstimatedTokens(parseInt(e.target.value) || 0)}
                  className="w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                />
              </Field>

              <Field
                label="Max Budget (USD, optional)"
                hint="Fails before network if estimated cost exceeds budget."
              >
                <input
                  type="number"
                  step="0.01"
                  placeholder="No limit"
                  value={budgetUsd}
                  onChange={(e) => setBudgetUsd(e.target.value)}
                  className="w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                />
              </Field>

              <Toggle
                checked={requireLocalOnly}
                onChange={setRequireLocalOnly}
                label="Use local models only"
                hint="Only consider models running on this computer."
              />

              <Button variant="primary" onClick={handleEvaluateRoute}>
                Preview selection
              </Button>

              {routingResult && (
                <div className="mt-3 rounded border border-ink-700 bg-ink-950 p-3 space-y-1.5 text-xs">
                  <div className="flex justify-between">
                    <span className="text-ink-400">Selected Model:</span>
                    <span className="font-semibold text-ink-100 font-mono">
                      {routingResult.selectedModelId}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-ink-400">Provider:</span>
                    <span className="text-ink-200">{routingResult.selectedProvider}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-ink-400">Routing Reason:</span>
                    <span className="text-ink-200">{routingResult.reason}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-ink-400">Estimated Cost:</span>
                    <span className="text-ink-200">
                      {typeof routingResult.estimatedCostUsd === "number"
                        ? `$${routingResult.estimatedCostUsd.toFixed(5)}`
                        : "N/A"}
                    </span>
                  </div>
                  <p className="pt-1 text-[11px] text-ink-500 italic border-t border-ink-800">
                    {routingResult.explanation}
                  </p>
                </div>
              )}
            </div>
          </Panel>

          <Panel
            title="Count tokens"
            description="Estimate how much text fits in a model’s context window."
          >
            <div className="space-y-3">
              <Field label="Sample Text" hint="Text to measure with tokenizer.">
                <textarea
                  rows={3}
                  value={sampleText}
                  onChange={(e) => setSampleText(e.target.value)}
                  className="w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                />
              </Field>

              <Toggle
                checked={exactCountOptIn}
                onChange={setExactCountOptIn}
                label="Allow online token counting"
                hint="May send this sample text to the provider. Leave off to estimate offline."
              />

              <Button variant="default" onClick={handleEstimateTokens}>
                Count Tokens
              </Button>

              {tokenResult && (
                <div className="mt-3 rounded border border-ink-700 bg-ink-950 p-3 space-y-1 text-xs">
                  <div className="flex justify-between">
                    <span className="text-ink-400">Token Count:</span>
                    <span className="font-semibold text-ink-100">{tokenResult.count} tokens</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-ink-400">Counting method:</span>
                    <span className="text-ink-200 mono text-[11px]">
                      {typeof tokenResult.estimateKind === "string"
                        ? tokenResult.estimateKind
                        : JSON.stringify(tokenResult.estimateKind)}
                    </span>
                  </div>
                </div>
              )}
            </div>
          </Panel>
        </div>

        {/* Section 4: Versioned Price Catalog */}
        {catalog && (
          <Panel
            title={`Model & Price Catalog (${catalog.version})`}
            description={`Updated: ${new Date(catalog.updatedAtMs).toLocaleDateString()} · Source: ${catalog.updateSource}. Pricing is versioned and transparent.`}
          >
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs text-ink-300">
                <thead className="border-b border-ink-800 text-[11px] text-ink-400">
                  <tr>
                    <th className="py-2">Model</th>
                    <th className="py-2">Provider</th>
                    <th className="py-2">Context Cap</th>
                    <th className="py-2">Input / 1M</th>
                    <th className="py-2">Output / 1M</th>
                    <th className="py-2">Cached Input / 1M</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-ink-800/60">
                  {catalog.entries.map((e) => {
                    const inputRate = e.pricing?.inputUsdPer1m ?? e.pricing?.inputUsdPer1M;
                    const outputRate = e.pricing?.outputUsdPer1m ?? e.pricing?.outputUsdPer1M;
                    const cachedRate =
                      e.pricing?.cachedInputUsdPer1m ?? e.pricing?.cachedInputUsdPer1M;
                    return (
                      <tr key={e.modelId}>
                        <td className="py-2 font-medium text-ink-100">{e.displayName}</td>
                        <td className="py-2">{e.provider}</td>
                        <td className="py-2">{formatNumber(e.contextCap)}</td>
                        <td className="py-2 mono">
                          {typeof inputRate === "number" ? `$${inputRate.toFixed(2)}` : "—"}
                        </td>
                        <td className="py-2 mono">
                          {typeof outputRate === "number" ? `$${outputRate.toFixed(2)}` : "—"}
                        </td>
                        <td className="py-2 mono">
                          {typeof cachedRate === "number" ? `$${cachedRate.toFixed(2)}` : "—"}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </Panel>
        )}
      </div>
    </div>
  );
}
