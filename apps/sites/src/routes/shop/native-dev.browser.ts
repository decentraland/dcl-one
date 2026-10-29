import RefreshRuntime from "virtual:react-router/hmr-runtime";

// A built Explorer host has no Vite HTML preamble. Install it before evaluating
// any JSX modules from the sites dev server (static imports would run too soon).
const host = window as typeof window & {
  $RefreshReg$?: (...args: unknown[]) => void;
  $RefreshSig$?: () => (type: unknown) => unknown;
  __vite_plugin_react_preamble_installed__?: boolean;
  __reactRouterDataRouter?: unknown;
};
RefreshRuntime.injectIntoGlobalHook(window);
host.$RefreshReg$ ??= () => {};
host.$RefreshSig$ ??= () => type => type;
host.__vite_plugin_react_preamble_installed__ = true;

// Framework HMR rebuilds the website router. Explorer owns a memory router
// instead, so invalidate this graph and reload on edits rather than invoking
// the framework's absent hydration globals. Ordinary website HMR stays intact.
const validate = RefreshRuntime.validateRefreshBoundaryAndEnqueueUpdate;
RefreshRuntime.validateRefreshBoundaryAndEnqueueUpdate = (...args) =>
  host.__reactRouterDataRouter
    ? validate(...args)
    : "Native Shop changed; reload the Explorer host";

export const { handle } = await import("../routes/shop-native");
