import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// The React Router Vite plugin guards client modules on the React Refresh preamble a browser
// page installs; jsdom tests have no page, so declare it before any component module loads.
(
  window as Window & { __vite_plugin_react_preamble_installed__?: boolean }
).__vite_plugin_react_preamble_installed__ = true;
// The same plugin instruments hook-calling functions in test files with these registration hooks.
Object.assign(window, {
  $RefreshReg$: () => undefined,
  $RefreshSig$: () => (type: unknown) => type,
});

// vitest globals are off in this app, so Testing Library cannot register its own cleanup.
afterEach(() => cleanup());
