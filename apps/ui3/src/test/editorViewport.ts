import { expect, vi } from "vitest";
import { waitFor } from "@testing-library/react";
import { BOOT_PROGRESS_POLL_MS, EDIT_VIEW_SETTLE_MS } from "../editor/editor-config";

export const EDIT_VIEW_REVEAL_MS = BOOT_PROGRESS_POLL_MS + EDIT_VIEW_SETTLE_MS;
export const EDIT_VIEW_WAIT = { timeout: EDIT_VIEW_REVEAL_MS + 3_000 };
export const OVERHEAD_CAMERA_POSE = { type: "camera-pose", x: 8, y: 24, z: -8, yaw: 0, pitch: 45 };

export async function useEditorClock(): Promise<void> {
  await import("../editor/components/DeInspectorPanel");
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] });
  vi.stubGlobal("jest", { advanceTimersByTime: (ms: number) => vi.advanceTimersByTime(ms) });
}

export function settleEngineWorld(frame: HTMLIFrameElement | null | undefined): void {
  if (!frame?.contentWindow) return;
  Object.assign(frame.contentWindow, {
    dclBridge: {
      onState: (listener: (push: unknown) => void) => {
        listener({ kind: "loading", ready: true, pendingAssets: 0 });
        return () => {};
      },
      send: () => {},
    },
  });
}

export async function editViewRevealed(): Promise<void> {
  await waitFor(() => {
    expect(document.querySelector(".eui-boot:not(.is-leaving)")).toBeNull();
    expect(document.querySelector('[aria-label="Undo"]')?.getAttribute("title")).not.toBe("Waiting for the engine to connect");
    expect(document.querySelector('[aria-label="Inspector loading"]')).toBeNull();
  }, EDIT_VIEW_WAIT);
}
