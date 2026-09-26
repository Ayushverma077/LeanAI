import "@testing-library/jest-dom/vitest";

// jsdom does not implement ResizeObserver, which the virtualised file tree
// uses to size its window.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
