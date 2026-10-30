import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import DclEditorChrome from "./DclEditorChrome";
import EditorBootShell from "./EditorBootShell";
import { bootClockReset } from "../boot-machine";
import { BOOT_LEAVE_MS, BOOT_PROGRESS_POLL_MS } from "../editor-config";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  bootClockReset();
});

describe("DclEditorChrome boot curtain", () => {
  const boot = (): HTMLElement | null => document.querySelector(".eui-boot");

  it("is one curtain from the route placeholder to the editor: same look, one running clock", () => {
    vi.useFakeTimers();
    const shell = render(<EditorBootShell />);
    const placeholder = boot();
    expect(placeholder?.getAttribute("role")).toBe("status");
    expect(placeholder?.querySelector(".eui-boot-spinner")).not.toBeNull();
    expect(placeholder?.textContent).toContain("Loading scene editor");
    act(() => { vi.advanceTimersByTime(3_000); });
    expect(placeholder?.querySelector(".eui-boot-elapsed")?.textContent).toBe("3s");
    shell.unmount();

    render(<DclEditorChrome loading viewportSrc={null} />);
    const curtain = boot();
    expect(curtain?.getAttribute("role")).toBe("status");
    expect(curtain?.querySelector(".eui-boot-spinner")).not.toBeNull();
    expect(curtain?.querySelector(".eui-boot-elapsed")?.textContent).toBe("3s");
    act(() => { vi.advanceTimersByTime(2_000); });
    expect(curtain?.querySelector(".eui-boot-elapsed")?.textContent).toBe("5s");
  });

  it("waits for the running engine instead of the pre-start Ready signal and observes bootstrap failure", () => {
    vi.useFakeTimers();
    const onBootstrap = vi.fn();
    const onEngineStatus = vi.fn();
    const viewportRef = { current: null as HTMLIFrameElement | null };
    render(<DclEditorChrome viewportSrc="/_play/?editorUi=1" viewportRef={viewportRef} onBootstrap={onBootstrap} onEngineStatus={onEngineStatus} />);
    const host = { dclEngineReady: true, dclBootstrapSnapshot: { phase: "Ready" } };
    Object.defineProperty(viewportRef.current!, "contentWindow", { configurable: true, value: host });
    act(() => { vi.advanceTimersByTime(BOOT_PROGRESS_POLL_MS); });
    expect(onBootstrap).toHaveBeenLastCalledWith(expect.objectContaining({ ready: false }));
    host.dclBootstrapSnapshot.phase = "World";
    act(() => { vi.advanceTimersByTime(BOOT_PROGRESS_POLL_MS); });
    expect(onBootstrap).toHaveBeenLastCalledWith(expect.objectContaining({ ready: true }));
    host.dclBootstrapSnapshot.phase = "Failed";
    act(() => { vi.advanceTimersByTime(BOOT_PROGRESS_POLL_MS); });
    expect(onEngineStatus).toHaveBeenLastCalledWith("offline");
  });

  it("treats iframe readiness as an engine fact when no scene readiness prop is supplied", () => {
    vi.useFakeTimers();
    const onBootstrap = vi.fn();
    const onEngineStatus = vi.fn();
    const viewportRef = { current: null as HTMLIFrameElement | null };
    render(
      <DclEditorChrome
        viewportSrc="/_play/?editorSession=00000000-0000-4000-8000-000000000001"
        viewportRef={viewportRef}
        onBootstrap={onBootstrap}
        onEngineStatus={onEngineStatus}
      />,
    );
    Object.defineProperty(viewportRef.current!, "contentWindow", {
      configurable: true,
      value: { dclEngineReady: true },
    });

    act(() => {
      vi.advanceTimersByTime(BOOT_PROGRESS_POLL_MS);
    });

    expect(onBootstrap).toHaveBeenLastCalledWith(expect.objectContaining({ ready: true }));
    expect(onEngineStatus).toHaveBeenLastCalledWith("connecting");
    expect(boot()?.textContent).toContain("Starting scene");
  });

  it("keeps the curtain until the edit view is ready and does not restore it during Run", () => {
    vi.useFakeTimers();
    const viewportRef = { current: null as HTMLIFrameElement | null };
    let emit: (push: unknown) => void = () => {};
    const { rerender } = render(
      <DclEditorChrome viewportSrc="/_play/?editorUi=1" viewportRef={viewportRef} sceneReady editViewReady={false} />,
    );
    Object.defineProperty(viewportRef.current!, "contentWindow", {
      configurable: true,
      value: {
        dclBootstrapSnapshot: { phase: "World" },
        dclBridge: { onState: (listener: (push: unknown) => void) => { emit = listener; return () => {}; }, send: () => {} },
      },
    });
    act(() => { vi.advanceTimersByTime(BOOT_PROGRESS_POLL_MS); });
    expect(boot()?.textContent).toContain("Preparing editor view");

    rerender(<DclEditorChrome viewportSrc="/_play/?editorUi=1" viewportRef={viewportRef} sceneReady editViewReady />);
    expect(boot()?.textContent).toContain("Preparing editor view");
    act(() => { vi.advanceTimersByTime(1200); });
    expect(boot()?.getAttribute("aria-hidden")).not.toBe("true");
    act(() => { emit({ kind: "loading", ready: true, pendingAssets: 0 }); });
    act(() => { vi.advanceTimersByTime(1199); });
    expect(boot()?.getAttribute("aria-hidden")).not.toBe("true");
    act(() => { vi.advanceTimersByTime(1); });
    expect(boot()?.getAttribute("aria-hidden")).toBe("true");
    act(() => { vi.advanceTimersByTime(BOOT_LEAVE_MS + 20); });
    expect(boot()).toBeNull();

    rerender(<DclEditorChrome viewportSrc="/_play/?editorUi=1" viewportRef={viewportRef} sceneReady editViewReady={false} />);
    expect(boot()).toBeNull();
  });

  it("shows the curtain while loading, plays a leave animation once loaded, then unmounts; a curtain never shown never leaves", () => {
    vi.useFakeTimers();
    const fresh = render(<DclEditorChrome loading={false} viewportSrc={null} />);
    expect(boot()?.getAttribute("aria-hidden")).not.toBe("true");
    fresh.unmount();

    const { rerender } = render(<DclEditorChrome loading viewportSrc={null} />);
    const shown = boot();
    expect(shown).not.toBeNull();
    expect(shown?.getAttribute("role")).toBe("status");

    rerender(<DclEditorChrome loading={false} viewportSrc={null} />);
    const leaving = boot();
    expect(leaving?.getAttribute("aria-hidden")).toBe("true");
    expect(leaving?.getAttribute("role")).toBeNull();

    act(() => {
      vi.advanceTimersByTime(BOOT_LEAVE_MS + 20);
    });
    expect(boot()).toBeNull();
  });
});
