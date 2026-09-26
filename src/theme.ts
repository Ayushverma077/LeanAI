/**
 * Theme selection.
 *
 * The palette lives entirely in CSS custom properties (`src/index.css`), so
 * switching themes is one attribute on `<html>` — no component knows which
 * theme is active. The `ink-*` scale keeps its *roles* in both themes
 * (`ink-950` is always the app background, `ink-100` is always the strongest
 * text), which is why existing markup works in light mode unchanged.
 */

export type Theme = "dark" | "light";

const STORAGE_KEY = "leanai.theme";

/** Reads the stored preference, falling back to the OS setting on first run. */
export function resolveInitialTheme(): Theme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "dark" || stored === "light") return stored;
  } catch {
    // Private mode or blocked storage: fall through to the OS preference.
  }
  if (typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: light)").matches) {
    return "light";
  }
  return "dark";
}

/** Applies the theme to the document and remembers the choice. */
export function applyTheme(theme: Theme, persist = true): void {
  document.documentElement.setAttribute("data-theme", theme);
  if (!persist) return;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Not being able to remember the choice is not worth failing over.
  }
}

// Applied at import time so the first paint is already in the right theme and
// the window never flashes dark before switching to light.
applyTheme(resolveInitialTheme(), false);
