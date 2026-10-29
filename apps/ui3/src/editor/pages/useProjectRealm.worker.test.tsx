import { afterEach, expect, test, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { useProjectRealm } from "./useProjectRealm";

vi.mock("../project-cache", async original => ({
  ...await original<object>(), setProjectPlayState: vi.fn(async () => {}),
}));
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

test.each(["/play/", "/_play/", "/custom-engine/"])("registers the viewport's own worker scope (%s)", async path => {
  vi.stubGlobal("caches", undefined);
  const register = vi.fn(async () => ({ active: {} }));
  vi.stubGlobal("navigator", { serviceWorker: { register, controller: null } });
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  const hook = renderHook(() => useProjectRealm(`${path}?realm=%2F_project`, null));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(hook.result.current).toBe("ready");
  expect(register).toHaveBeenCalledWith(`${path}service_worker.js`, { scope: path });
  hook.unmount();
});

test("does not register a different origin's engine worker on the editor origin", async () => {
  vi.stubGlobal("caches", undefined);
  const register = vi.fn();
  vi.stubGlobal("navigator", { serviceWorker: { register, controller: null } });
  const hook = renderHook(() => useProjectRealm("https://engine.test/play/?realm=%2F_project", null));
  await waitFor(() => expect(hook.result.current).toBe("ready"));
  expect(register).not.toHaveBeenCalled();
  hook.unmount();
});
