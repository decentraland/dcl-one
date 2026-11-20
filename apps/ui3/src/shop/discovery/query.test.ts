// The prefetch cache's ordering: an answer only lands while its read is still current.

import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { invalidateShopQueries, peekShopQuery, prefetchShopQuery, useShopQuery } from "./query";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => invalidateShopQueries());

describe("prefetchShopQuery", () => {
  it("stores the answer of a read that is still current", async () => {
    const a = deferred<string>();
    prefetchShopQuery(["rail"], () => a.promise);
    a.resolve("A");
    await settle();
    expect(peekShopQuery(["rail"])).toBe("A");
  });

  it("never lets a read invalidated meanwhile replace a later read's answer", async () => {
    const a = deferred<string>();
    const b = deferred<string>();
    prefetchShopQuery(["rail"], () => a.promise);
    invalidateShopQueries();
    prefetchShopQuery(["rail"], () => b.promise);
    b.resolve("B");
    await settle();
    expect(peekShopQuery(["rail"])).toBe("B");
    a.resolve("A");
    await settle();
    expect(peekShopQuery(["rail"])).toBe("B");
  });

  it("never lets the read a page joined replace the page's later read once invalidated", async () => {
    const a = deferred<string>();
    const b = deferred<string>();
    prefetchShopQuery(["rail"], () => a.promise);
    const { result, unmount } = renderHook(() => useShopQuery(["rail"], () => b.promise));
    act(() => invalidateShopQueries());
    await act(async () => {
      b.resolve("B");
      await settle();
    });
    expect(peekShopQuery(["rail"])).toBe("B");
    await act(async () => {
      a.resolve("A");
      await settle();
    });
    expect(peekShopQuery(["rail"])).toBe("B");
    expect(result.current.data).toBe("B");
    unmount();
  });
});
