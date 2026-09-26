import { Component, useEffect, useState, type ErrorInfo, type ReactNode } from "react";

import { CommandPalette } from "./components/layout/CommandPalette";
import { Sidebar } from "./components/layout/Sidebar";
import { StatusBar } from "./components/layout/StatusBar";
import { TopBar } from "./components/layout/TopBar";
import { Button } from "./components/primitives";
import { onScanProgress } from "./ipc/client";
import { AgentsPage } from "./pages/AgentsPage";
import { ContextBundlerPage } from "./pages/ContextBundlerPage";
import { HistoryPage } from "./pages/HistoryPage";
import { ModelsPage } from "./pages/ModelsPage";
import { OverviewPage } from "./pages/OverviewPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TasksPage } from "./pages/TasksPage";
import { useAppStore } from "./store/useAppStore";

export default function App() {
  const { route, bootstrap, error, setError, notice, setNotice, setScanProgress } = useAppStore();

  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    const unlisten = onScanProgress(setScanProgress);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [setScanProgress]);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), 6000);
    return () => clearTimeout(timer);
  }, [notice, setNotice]);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-ink-950 text-ink-100 antialiased select-none">
      {/* Accessible skip link */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:m-2 focus:rounded-md focus:bg-white focus:px-3 focus:py-1.5 focus:text-xs focus:text-ink-950 focus:font-medium focus:shadow-lg"
      >
        Skip to main content
      </a>

      {/* Top Bar with Project Switcher, Token Gauge, and Global Shortcuts */}
      <TopBar onOpenCommandPalette={() => setCommandPaletteOpen(true)} />

      {/* Error & Notice Notification Alerts */}
      <div aria-live="polite" className="empty:hidden z-40">
        {error ? (
          <div
            role="alert"
            className="flex items-start justify-between gap-3 border-b border-danger/40 bg-danger/10 px-4 py-2 text-xs text-danger shadow-sm backdrop-blur-md"
          >
            <div>
              <p className="font-semibold">{error.message}</p>
              {error.recovery ? <p className="mt-0.5 text-danger/80">{error.recovery}</p> : null}
              <p className="mono mt-0.5 text-[10px] text-danger/60">{error.code}</p>
            </div>
            <Button variant="ghost" size="xs" onClick={() => setError(null)}>
              Dismiss
            </Button>
          </div>
        ) : null}
        {notice ? (
          <div
            className="flex items-center justify-between border-b border-ok/30 bg-ok/10 px-4 py-1.5 text-xs font-medium text-ok shadow-sm backdrop-blur-md"
            role="status"
          >
            <span>{notice}</span>
            <button
              type="button"
              onClick={() => setNotice(null)}
              className="text-ok/70 hover:text-ok text-[11px]"
            >
              ✕
            </button>
          </div>
        ) : null}
      </div>

      {/* Main Studio Body: Sidebar + Dynamic Workspace Area */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        <Sidebar />

        <main id="main-content" className="flex-1 overflow-y-auto p-4 bg-ink-950">
          <ErrorBoundary>
            {route === "overview" || route === "workspace" ? <OverviewPage /> : null}
            {route === "context" || route === "select" || route === "preview" ? (
              <ContextBundlerPage />
            ) : null}
            {route === "agents" ? <AgentsPage /> : null}
            {route === "tasks" ? <TasksPage /> : null}
            {route === "history" ? <HistoryPage /> : null}
            {route === "models" ? <ModelsPage /> : null}
            {route === "settings" ? <SettingsPage /> : null}
          </ErrorBoundary>
        </main>
      </div>

      {/* Status Bar */}
      <StatusBar />

      {/* Global Command Palette (⌘K) */}
      <CommandPalette isOpen={commandPaletteOpen} onClose={() => setCommandPaletteOpen(false)} />
    </div>
  );
}

/** Error Boundary: Keeps a component failure from crashing the desktop window. */
class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("LeanAi Desktop UI error:", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="m-4 rounded-xl border border-danger/40 bg-danger/10 p-5 text-xs text-danger">
        <p className="text-sm font-bold">This screen failed to render.</p>
        <p className="mt-1 leading-relaxed text-danger/90">
          Your project files and settings are completely unaffected — nothing was written or lost.
          Reload the window or switch to another section.
        </p>
        <pre className="mono mt-3 overflow-auto rounded bg-black/40 p-3 text-[11px] text-danger/80">
          {this.state.error.message}
        </pre>
        <div className="mt-4">
          <Button variant="danger" onClick={() => this.setState({ error: null })}>
            Try again
          </Button>
        </div>
      </div>
    );
  }
}
