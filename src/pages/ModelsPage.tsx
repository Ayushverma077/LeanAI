import { useCallback, useEffect, useState } from "react";

import { Button, Chip, Field, Panel, Toggle, formatNumber } from "../components/primitives";
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
      setNotice(`Registered local model: ${newModelName}`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setRegistering(false);
    }
  };

  const handleUnregisterModel = async (id: string, name: string) => {
    try {
      await api.unregisterModel(id);
      setNotice(`Unregistered model: ${name}`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const handleStartSidecar = async (id: string) => {
    try {
      const status = await api.startLocalModel(id);
      setSidecarStatus(status);
      setNotice("Local model sidecar started successfully on loopback.");
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const handleStopSidecar = async () => {
    try {
      const status = await api.stopLocalModel();
      setSidecarStatus(status);
      setNotice("Local model sidecar stopped.");
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const handleConfigureCredential = async (providerId: string) => {
    if (!credentialSecret.trim()) return;
    try {
      await api.configureProviderCredential(providerId, "api_key", credentialSecret.trim());
      setConfiguringProvider(null);
      setCredentialSecret("");
      setNotice(`Credential securely stored in OS keychain for ${providerId}.`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
    }
  };

  const handleDisconnectProvider = async (providerId: string) => {
    try {
      await api.disconnectProvider(providerId);
      setNotice(`Disconnected ${providerId} and removed key from OS keychain.`);
      await refreshData();
    } catch (err) {
      setError(toAppError(err));
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

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-base font-semibold text-ink-100">Models & Providers</h1>
        <p className="mt-1 text-xs text-ink-400">
          Configure local GGUF execution via the loopback <code>llama-server</code> sidecar (Phase
          6) and cloud providers with OS secure credential storage, capability profiles, and
          cost-aware routing (Phase 7).
        </p>
      </div>

      {/* Section 1: Local Model Runtime */}
      <Panel
        title="Local Model Runtime (llama-server Sidecar)"
        description="Run local inference without exposing a network listener. Binds loopback strictly (127.0.0.1) on an ephemeral port with zero orphan processes."
        actions={
          sidecarStatus.state === "ready" ? (
            <Button variant="danger" onClick={handleStopSidecar}>
              Stop Sidecar
            </Button>
          ) : (
            <Button variant="ghost" onClick={refreshData}>
              Refresh Status
            </Button>
          )
        }
      >
        <div className="space-y-4">
          <div className="flex items-center gap-3 rounded border border-ink-800 bg-ink-950 p-3">
            <span className="text-xs font-medium text-ink-300">Runtime Status:</span>
            {sidecarStatus.state === "ready" && (
              <Chip tone="ok" title="Active on loopback port">
                Ready · Port {sidecarStatus.port} · PID {sidecarStatus.pid} (
                {sidecarStatus.displayName})
              </Chip>
            )}
            {sidecarStatus.state === "starting" && (
              <Chip tone="warn" title="Initializing on loopback port">
                Starting on port {sidecarStatus.port}…
              </Chip>
            )}
            {sidecarStatus.state === "stopped" && (
              <Chip tone="neutral" title="Sidecar process is stopped">
                Stopped
              </Chip>
            )}
            {sidecarStatus.state === "stopping" && (
              <Chip tone="warn" title="Terminating child process">
                Stopping…
              </Chip>
            )}
            {sidecarStatus.state === "error" && (
              <Chip tone="danger" title={sidecarStatus.message}>
                Error: {sidecarStatus.message}
              </Chip>
            )}
          </div>

          <div className="space-y-2">
            <h3 className="text-xs font-semibold text-ink-200">Registered GGUF Models</h3>
            {models.length === 0 ? (
              <p className="text-xs text-ink-500 italic">No local GGUF models registered yet.</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs text-ink-300">
                  <thead className="border-b border-ink-800 text-[11px] text-ink-400">
                    <tr>
                      <th className="py-2">Name</th>
                      <th className="py-2">Architecture</th>
                      <th className="py-2">Context Cap</th>
                      <th className="py-2">Path</th>
                      <th className="py-2 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-ink-800/60">
                    {models.map((m) => (
                      <tr key={m.id}>
                        <td className="py-2 font-medium text-ink-100">{m.displayName}</td>
                        <td className="py-2 mono text-[11px]">{m.source}</td>
                        <td className="py-2">
                          {formatNumber(m.capabilityProfile.contextCap)} tokens
                        </td>
                        <td
                          className="py-2 mono text-[10px] text-ink-500 max-w-[200px] truncate"
                          title={m.filePath ?? ""}
                        >
                          {m.filePath}
                        </td>
                        <td className="py-2 text-right space-x-2">
                          <Button
                            variant="primary"
                            disabled={
                              sidecarStatus.state === "ready" && sidecarStatus.modelId === m.id
                            }
                            onClick={() => handleStartSidecar(m.id)}
                          >
                            {sidecarStatus.state === "ready" && sidecarStatus.modelId === m.id
                              ? "Running"
                              : "Start"}
                          </Button>
                          <Button
                            variant="ghost"
                            onClick={() => handleUnregisterModel(m.id, m.displayName)}
                          >
                            Remove
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <form
            onSubmit={handleRegisterModel}
            className="rounded border border-ink-800 bg-ink-950/60 p-3 space-y-3"
          >
            <h4 className="text-xs font-medium text-ink-200">Register New GGUF Model</h4>
            <div className="grid gap-3 sm:grid-cols-3">
              <div>
                <label className="block text-[11px] text-ink-400">Display Name</label>
                <input
                  type="text"
                  placeholder="e.g. Llama 3.2 3B Instruct"
                  value={newModelName}
                  onChange={(e) => setNewModelName(e.target.value)}
                  className="mt-1 w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                  required
                />
              </div>
              <div className="sm:col-span-2">
                <label className="block text-[11px] text-ink-400">GGUF File Path</label>
                <input
                  type="text"
                  placeholder="/absolute/path/to/model.gguf"
                  value={newModelPath}
                  onChange={(e) => setNewModelPath(e.target.value)}
                  className="mt-1 w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                  required
                />
              </div>
            </div>
            <div className="flex items-center justify-between pt-1">
              <div className="flex items-center gap-2">
                <label className="text-[11px] text-ink-400">Context Cap:</label>
                <input
                  type="number"
                  value={newModelContext}
                  onChange={(e) => setNewModelContext(parseInt(e.target.value) || 8192)}
                  className="w-24 rounded border border-ink-700 bg-ink-900 px-2 py-1 text-xs text-ink-100"
                />
              </div>
              <Button type="submit" variant="primary" disabled={registering}>
                {registering ? "Inspecting GGUF Header…" : "Inspect & Register Model"}
              </Button>
            </div>
          </form>
        </div>
      </Panel>

      {/* Section 2: Cloud Providers & OS Keychain */}
      <Panel
        title="Cloud Providers & OS Secure Credential Storage"
        description="API keys live strictly in your platform OS keychain (macOS Keychain). SQLite stores only a non-secret reference (is_configured). Credentials never touch database files, logs, or diagnostic bundles (ADR 0007)."
      >
        <div className="space-y-3">
          <div className="divide-y divide-ink-800">
            {providers.map((p) => (
              <div key={p.providerId} className="flex items-center justify-between py-3">
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-semibold text-ink-100">{p.displayName}</span>
                    {p.isConfigured ? (
                      <Chip tone="ok">Configured in Keychain</Chip>
                    ) : (
                      <Chip tone="neutral">Not Configured</Chip>
                    )}
                  </div>
                  <p className="mt-0.5 text-[11px] text-ink-500">
                    Catalog version: {p.priceCatalogVer} · Identifier: {p.providerId}
                  </p>
                </div>
                <div className="flex gap-2">
                  {p.isConfigured ? (
                    <Button variant="danger" onClick={() => handleDisconnectProvider(p.providerId)}>
                      Disconnect
                    </Button>
                  ) : (
                    <Button variant="primary" onClick={() => setConfiguringProvider(p.providerId)}>
                      Configure Key
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>

          {configuringProvider && (
            <div className="rounded border border-ink-750 bg-ink-950 p-3.5 space-y-2.5">
              <h4 className="text-xs font-semibold text-white">
                Configure API Credential for {configuringProvider}
              </h4>
              <p className="text-[11px] text-ink-400">
                This secret will be stored directly into your operating system's secure keychain
                under <code>dev.leanai.desktop.{configuringProvider}</code>. It is never stored in
                SQLite.
              </p>
              <input
                type="password"
                placeholder="Enter API Key / Token"
                value={credentialSecret}
                onChange={(e) => setCredentialSecret(e.target.value)}
                className="w-full rounded border border-ink-700 bg-ink-900 px-3 py-1.5 text-xs text-ink-100"
              />
              <div className="flex justify-end gap-2">
                <Button
                  variant="ghost"
                  onClick={() => {
                    setConfiguringProvider(null);
                    setCredentialSecret("");
                  }}
                >
                  Cancel
                </Button>
                <Button
                  variant="primary"
                  onClick={() => handleConfigureCredential(configuringProvider)}
                >
                  Save to Keychain
                </Button>
              </div>
            </div>
          )}
        </div>
      </Panel>

      {/* Section 3: Cost-Aware Routing & Exact Token Counting */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel
          title="Cost-Aware Routing Simulator"
          description="Deterministic task-based model selection with automatic escalation and hard budget ceilings (ADR 0008)."
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
                <option value="multi_file_planning">Multi-File Planning (Strong Reasoning)</option>
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
              label="Require Local Only (GGUF Sidecar)"
              hint="Restricts inference strictly to local machine with zero external egress."
            />

            <Button variant="primary" onClick={handleEvaluateRoute}>
              Evaluate Route
            </Button>

            {routingResult && (
              <div className="mt-3 rounded border border-ink-700 bg-ink-950 p-3 space-y-1.5 text-xs">
                <div className="flex justify-between">
                  <span className="text-ink-400">Selected Model:</span>
                  <span className="font-semibold text-white font-mono">
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
          title="Exact Token Counting (FR-14)"
          description="Exact count endpoints require explicit user opt-in before any network call. Otherwise LeanAI falls back to local BPE estimation."
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
              label="Exact Provider Token Count Opt-In"
              hint="Off by default (FR-14). When disabled, uses local offline tokenizer estimate."
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
                  <span className="text-ink-400">Provenance Label:</span>
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
  );
}
