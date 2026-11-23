import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useReadOnClose } from "./useReadOnClose";

describe("useReadOnClose", () => {
  it("reads once when the dialog closes after a purchase went out, and not again on unmount", () => {
    const read = vi.fn();
    const { result, unmount } = renderHook(() => useReadOnClose(read));
    act(() => result.current.arm());
    expect(read).not.toHaveBeenCalled();
    act(() => result.current.fire());
    expect(read).toHaveBeenCalledOnce();
    unmount();
    expect(read).toHaveBeenCalledOnce();
  });

  it("reads when the page unmounts before the dialog closed, so the refresh is not lost", () => {
    const read = vi.fn();
    const { result, unmount } = renderHook(() => useReadOnClose(read));
    act(() => result.current.arm());
    unmount();
    expect(read).toHaveBeenCalledOnce();
  });

  it("reads nothing when no purchase went out", () => {
    const read = vi.fn();
    const { result, unmount } = renderHook(() => useReadOnClose(read));
    act(() => result.current.fire());
    unmount();
    expect(read).not.toHaveBeenCalled();
  });

  it("reads with the page's latest reader", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { result, rerender, unmount } = renderHook(({ read }) => useReadOnClose(read), { initialProps: { read: first } });
    act(() => result.current.arm());
    rerender({ read: second });
    unmount();
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
  });

  it("reads once, at once, when the purchase's outcome arrives after the page unmounted", () => {
    const read = vi.fn();
    const { result, unmount } = renderHook(() => useReadOnClose(read));
    const { arm } = result.current;
    // Back while the sale's receipt was awaited: nothing armed yet, so the unmount reads nothing.
    unmount();
    expect(read).not.toHaveBeenCalled();
    arm();
    expect(read).toHaveBeenCalledOnce();
    arm();
    expect(read).toHaveBeenCalledOnce();
  });
});
