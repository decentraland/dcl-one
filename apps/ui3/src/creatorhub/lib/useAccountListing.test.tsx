import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { useAccountListing } from "./useAccountListing";

afterEach(cleanup);

function loads() {
  const calls: Array<(value: string[]) => void> = [];
  const load = vi.fn(() => new Promise<string[]>((resolve) => calls.push(resolve)));
  return { load, answer: (call: number, value: string[]) => act(async () => calls[call]!(value)) };
}

it("lists again on sign-in, sign-out and an account switch, and never shows the last account's list", async () => {
  const { load, answer } = loads();
  const hook = renderHook(({ owner }) => useAccountListing(owner, load), { initialProps: { owner: null as string | null } });
  await answer(0, []);
  expect(hook.result.current.value).toEqual([]);

  hook.rerender({ owner: "0xa" });
  expect(hook.result.current.value).toBeNull();
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await answer(1, ["a-plaza", "a-market"]);
  expect(hook.result.current.value).toEqual(["a-plaza", "a-market"]);

  act(() => window.dispatchEvent(new Event("focus")));
  hook.rerender({ owner: "0xb" });
  expect(hook.result.current.value).toBeNull();
  await waitFor(() => expect(load).toHaveBeenCalledTimes(4));
  await answer(2, ["a-plaza", "a-market"]);
  expect(hook.result.current.value).toBeNull();
  await answer(3, []);
  expect(hook.result.current.value).toEqual([]);

  hook.rerender({ owner: null });
  expect(hook.result.current.value).toBeNull();
  await waitFor(() => expect(load).toHaveBeenCalledTimes(5));
});

it("loads again on focus and on reload, keeping only the newest answer, and applies local changes", async () => {
  const { load, answer } = loads();
  const hook = renderHook(() => useAccountListing("0xa", load));
  await answer(0, ["plaza"]);
  act(() => window.dispatchEvent(new Event("focus")));
  act(() => hook.result.current.reload());
  await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
  await answer(2, ["plaza", "market"]);
  await answer(1, ["stale"]);
  expect(hook.result.current.value).toEqual(["plaza", "market"]);

  act(() => hook.result.current.update((list) => [...list, "copy"]));
  expect(hook.result.current.value).toEqual(["plaza", "market", "copy"]);
});
