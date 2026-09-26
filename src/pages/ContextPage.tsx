import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";

import { Button, Chip, EmptyState, Panel } from "../components/primitives";
import { api, toAppError } from "../ipc/client";
import type { ContextSection, SourceOnDemandResponse } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";

export function ContextPage() {
  const store = useAppStore();
  const { context, inventory } = store;
  const [openSection, setOpenSection] = useState<string | null>("metadata");
  const [source, setSource] = useState<SourceOnDemandResponse | null>(null);

  if (!inventory) {
    return (
      <EmptyState
        title="Scan a project first"
        body="The context index is generated from a scan, so it always has a revision to point at."
        action={<Button onClick={() => store.setRoute("workspace")}>Go to the project</Button>}
      />
    );
  }

  if (!context) {
    return (
      <EmptyState
        title="No context index yet"
        body={
          <>
            LeanAI builds a Markdown index of this project — structure, modules, dependencies,
            routes, configuration and TODOs — where every section records the files and content
            hashes it was generated from. It is written by ordinary code, not a model, and it does
            not replace reading the source.
          </>
        }
        action={
          <Button variant="primary" onClick={store.generateContext}>
            Generate context index
          </Button>
        }
      />
    );
  }

  const tone =
    context.freshness === "fresh" ? "ok" : context.freshness === "stale" ? "warn" : "neutral";

  return (
    <div className="space-y-4">
      <Panel
        title="PROJECT_CONTEXT.md"
        description={
          <>
            Schema v{context.document.schemaVersion} · generated at revision{" "}
            <span className="mono">{context.document.sourceRevision.slice(0, 24)}</span>
          </>
        }
        actions={
          <>
            <Button onClick={store.generateContext}>Regenerate</Button>
            <Button
              variant="primary"
              onClick={async () => {
                const target = await save({
                  title: "Save context index",
                  defaultPath: "PROJECT_CONTEXT.md",
                  filters: [{ name: "Markdown", extensions: ["md"] }],
                });
                if (!target) return;
                try {
                  await api.saveContextDocument(target);
                  store.setNotice(`Saved to ${target}.`);
                } catch (error) {
                  store.setError(toAppError(error));
                }
              }}
            >
              Save as…
            </Button>
          </>
        }
      >
        <div className="flex flex-wrap gap-2">
          <Chip tone={tone}>
            {context.freshness === "fresh"
              ? "✓ every cited source still matches"
              : context.freshness === "stale"
                ? "⚠ stale — sources changed since generation"
                : "? freshness cannot be verified for some sources"}
          </Chip>
          <Chip>{context.document.sections.length} sections</Chip>
          {context.staleSectionKeys.length > 0 ? (
            <Chip tone="warn">{context.staleSectionKeys.length} need re-verifying</Chip>
          ) : null}
        </div>
        {context.freshness !== "fresh" ? (
          <p className="mt-3 rounded border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
            A stale section may describe code that no longer exists. Regenerate, or open the source
            behind any claim before relying on it.
          </p>
        ) : null}
      </Panel>

      <div className="space-y-2">
        {context.document.sections.map((section) => (
          <SectionCard
            key={section.key}
            section={section}
            open={openSection === section.key}
            onToggle={() => setOpenSection(openSection === section.key ? null : section.key)}
            onOpenSource={async (path) => {
              try {
                setSource(await api.fetchSource(path));
              } catch (error) {
                store.setError(toAppError(error));
              }
            }}
          />
        ))}
      </div>

      {source ? <SourceDialog source={source} onClose={() => setSource(null)} /> : null}
    </div>
  );
}

function SectionCard({
  section,
  open,
  onToggle,
  onOpenSource,
}: {
  section: ContextSection;
  open: boolean;
  onToggle: () => void;
  onOpenSource: (path: string) => void;
}) {
  const tone =
    section.freshness === "fresh" ? "ok" : section.freshness === "stale" ? "warn" : "neutral";
  return (
    <section className="rounded-lg border border-ink-800 bg-ink-900">
      <h2>
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={open}
          className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left"
        >
          <span className="text-sm font-semibold text-ink-100">{section.title}</span>
          <span className="flex shrink-0 items-center gap-2">
            <Chip tone={tone}>{section.freshness}</Chip>
            <Chip tone={section.generator.generator === "model" ? "info" : "neutral"}>
              {section.generator.generator === "model"
                ? `written by ${section.generator.provider}/${section.generator.model}`
                : "deterministic"}
            </Chip>
            <span className="text-ink-500">{open ? "▾" : "▸"}</span>
          </span>
        </button>
      </h2>
      {open ? (
        <div className="border-t border-ink-800 px-4 py-3">
          <pre className="mono max-h-96 overflow-auto text-[11px] leading-relaxed whitespace-pre-wrap text-ink-300">
            {section.body}
          </pre>
          {section.limitations.length > 0 ? (
            <div className="mt-3 rounded border border-ink-800 bg-ink-950 px-3 py-2">
              <p className="text-[11px] font-semibold text-ink-300">
                What this section cannot tell you
              </p>
              <ul className="mt-1 list-disc space-y-0.5 pl-4 text-[11px] text-ink-500">
                {section.limitations.map((limitation) => (
                  <li key={limitation}>{limitation}</li>
                ))}
              </ul>
            </div>
          ) : null}
          {section.sourceRefs.length > 0 ? (
            <div className="mt-3">
              <p className="text-[11px] font-semibold text-ink-300">
                Sources ({section.sourceRefs.length}) — open one to read the current file
              </p>
              <ul className="mt-1 flex flex-wrap gap-1">
                {section.sourceRefs.map((ref) => (
                  <li key={ref.path}>
                    <button
                      type="button"
                      onClick={() => onOpenSource(ref.path)}
                      className="mono rounded border border-ink-800 px-1.5 py-0.5 text-[11px] text-ink-300 hover:border-ink-600 hover:text-white transition-colors"
                    >
                      {ref.path}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

/**
 * Source-on-demand: the mechanism that keeps a summary from becoming the source
 * of truth. The dialog states plainly whether the file changed since the scan
 * the claim was generated against (FR-20).
 */
function SourceDialog({
  source,
  onClose,
}: {
  source: SourceOnDemandResponse;
  onClose: () => void;
}) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="source-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
    >
      <div className="max-h-[85vh] w-full max-w-3xl overflow-auto rounded-lg border border-ink-700 bg-ink-900 p-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 id="source-title" className="mono text-sm text-ink-100">
              {source.path}
            </h2>
            <p className="mt-1 text-[11px] text-ink-500">
              lines {source.fromLine}–{source.toLine} of {source.totalLines} · sha256{" "}
              {source.contentHash.slice(0, 16)}…
            </p>
          </div>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </div>
        {source.changedSinceScan ? (
          <p className="mt-3 rounded border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
            This file has changed since the last scan. What you see below is the current content,
            not what the context section was written from.
          </p>
        ) : null}
        <pre className="mono mt-3 max-h-[60vh] overflow-auto rounded border border-ink-800 bg-ink-950 p-3 text-[11px] whitespace-pre-wrap text-ink-300">
          {source.content}
        </pre>
      </div>
    </div>
  );
}
