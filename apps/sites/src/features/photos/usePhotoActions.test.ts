// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";

import { usePhotoActions } from "./usePhotoActions";

const photo = {
  id: "11111111-1111-4111-8111-111111111111",
  url: "https://photos.example/photo.png",
  author: { name: "Alice", address: "" },
  dateTime: "2026-11-20",
} as ProfilePhoto;
const shareUrl = `https://reels.example/reels/${photo.id}`;

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("when sharing from the configured public origin", () => {
  let writeText: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    writeText = vi.fn();
    vi.stubGlobal("navigator", { clipboard: { writeText } });
  });

  it("should copy the same direct URL advertised by the X link", async () => {
    writeText.mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => usePhotoActions(photo, true, "https://reels.example"));
    await act(async () => result.current.onCopy?.());
    expect(writeText).toHaveBeenCalledWith(shareUrl);
    expect(new URL(result.current.shareHref!).searchParams.get("url")).toBe(shareUrl);
  });

  it("should announce another successful copy and dismiss temporary feedback", async () => {
    vi.useFakeTimers();
    writeText.mockResolvedValue(undefined);
    const { result } = renderHook(() => usePhotoActions(photo, true, "https://reels.example"));
    await act(async () => result.current.onCopy?.());
    const firstAnnouncement = result.current.announcementVersion;
    await act(async () => result.current.onCopy?.());
    expect(result.current.announcementVersion).not.toBe(firstAnnouncement);
    act(() => vi.advanceTimersByTime(5_000));
    expect(result.current.announcement).toBeUndefined();
  });

  it("should expose the direct URL as fallback when clipboard access fails", async () => {
    writeText.mockRejectedValueOnce(new Error("Denied"));
    const { result } = renderHook(() => usePhotoActions(photo, true, "https://reels.example"));
    await act(async () => result.current.onCopy?.());
    expect(result.current.copyFallbackHref).toBe(shareUrl);
  });
});

describe("when downloading a photo", () => {
  let fetchImage: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchImage = vi.fn();
    vi.stubGlobal("fetch", fetchImage);
  });

  it("should cancel the unused body of a failed response and announce the failure", async () => {
    const cancel = vi.fn().mockResolvedValue(undefined);
    fetchImage.mockResolvedValueOnce({ ok: false, body: { cancel } });
    const { result } = renderHook(() => usePhotoActions(photo, true));
    await act(async () => result.current.onDownload?.());
    expect(cancel).toHaveBeenCalledOnce();
    expect(result.current.announcement).toMatch(/could not download/i);
  });

  it("should reject a successful response carrying a non-image MIME type", async () => {
    fetchImage.mockResolvedValueOnce(new Response("<html>", { headers: { "Content-Type": "text/html" } }));
    const { result } = renderHook(() => usePhotoActions(photo, true));
    await act(async () => result.current.onDownload?.());
    expect(result.current.announcement).toMatch(/could not download/i);
  });

  it("should ignore duplicate activation and abort when the photo changes", async () => {
    fetchImage.mockImplementation(
      (_url, { signal }: { signal: AbortSignal }) =>
        new Promise((_, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true })),
    );
    const { result, rerender } = renderHook(({ current }) => usePhotoActions(current, true), {
      initialProps: { current: photo },
    });
    act(() => {
      result.current.onDownload?.();
      result.current.onDownload?.();
    });
    expect(fetchImage).toHaveBeenCalledOnce();
    await act(async () => rerender({ current: { ...photo, id: "22222222-2222-4222-8222-222222222222" } }));
    expect(fetchImage.mock.calls[0]?.[1].signal.aborted).toBe(true);
    expect(result.current.announcement).toBeUndefined();
  });
});
