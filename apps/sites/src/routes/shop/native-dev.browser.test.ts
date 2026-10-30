import { afterEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ inject: vi.fn(), validate: vi.fn(), handle: { version: 1 } }));
vi.mock("virtual:react-router/hmr-runtime", () => ({ default: {
  injectIntoGlobalHook: mocks.inject,
  validateRefreshBoundaryAndEnqueueUpdate: mocks.validate,
} }));
vi.mock("../routes/shop-native", () => {
  expect((window as unknown as Record<string, unknown>).__vite_plugin_react_preamble_installed__).toBe(true);
  expect(mocks.inject).toHaveBeenCalledWith(window);
  return { handle: mocks.handle };
});
afterEach(() => vi.unstubAllGlobals());

it("prepares an uninitialized Explorer before importing routes and avoids framework HMR without its router", async () => {
  const host: Record<string, unknown> = {};
  vi.stubGlobal("window", host);
  const entry = await import("./native-dev.browser");
  expect(entry.handle).toBe(mocks.handle);
  expect(host.$RefreshReg$).toBeTypeOf("function");
  expect(host.$RefreshSig$).toBeTypeOf("function");
  const { default: refresh } = await import("virtual:react-router/hmr-runtime");
  expect(refresh.validateRefreshBoundaryAndEnqueueUpdate({}, {})).toMatch(/reload/i);
  expect(mocks.validate).not.toHaveBeenCalled();

  // A regular website host keeps the framework's refresh behavior.
  host.__reactRouterDataRouter = {};
  refresh.validateRefreshBoundaryAndEnqueueUpdate("before", "after");
  expect(mocks.validate).toHaveBeenCalledWith("before", "after");
});
