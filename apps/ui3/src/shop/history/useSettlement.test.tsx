import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { createDemo, DemoShell } from "../checkout/demo";
import { useSettlement } from "./useSettlement";

// The ownership read, held by the test: each call keeps the signal it was given and never answers on its own.
const ownership = vi.hoisted(() => ({ signals: [] as (AbortSignal | undefined)[] }));
vi.mock("../checkout/lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../checkout/lib/api")>()),
  fetchOwnsItem: (_owner: string, _contract: string, _itemId: string, _client: unknown, signal?: AbortSignal) => {
    ownership.signals.push(signal);
    return new Promise<boolean>(() => {});
  },
}));

describe("useSettlement", () => {
  it("stops reading the receipt once the page that follows it is gone", async () => {
    const demo = createDemo({ session: "web3" });
    // Real waits between reads, so a loop left running would keep reading after the unmount.
    demo.services.wait = { timeoutMs: 60_000, pollMs: 5 };
    let reads = 0;
    const reader = demo.services.reader!;
    const receipt = reader.receipt.bind(reader);
    reader.receipt = (chainId, txHash, signal) => {
      reads++;
      return receipt(chainId, txHash, signal);
    };
    const wrapper = ({ children }: { children: ReactNode }) => (
      <DemoShell demo={demo} path="/shop/history">
        {children}
      </DemoShell>
    );
    const tx = { hash: `0x${"e".repeat(64)}`, chainId: 137 };
    const view = renderHook(() => useSettlement([tx]), { wrapper });
    await waitFor(() => expect(reads).toBeGreaterThanOrEqual(2));
    expect(view.result.current).toBe("pending");

    view.unmount();
    const atUnmount = reads;
    await new Promise((settled) => setTimeout(settled, 60));
    expect(reads).toBe(atUnmount);
  });

  it("stops its ownership read once the page that follows it is gone", async () => {
    ownership.signals.length = 0;
    const demo = createDemo({ session: "web3" });
    const tx = { hash: `0x${"9".repeat(64)}`, chainId: 137 };
    demo.chain.receipts.set(tx.hash, "success");
    const wrapper = ({ children }: { children: ReactNode }) => (
      <DemoShell demo={demo} path="/shop/history">
        {children}
      </DemoShell>
    );
    const own = { owner: `0x${"2".repeat(40)}`, contractAddress: `0x${"3".repeat(40)}`, itemId: "7" };
    const view = renderHook(() => useSettlement([tx], { ownership: own }), { wrapper });
    await waitFor(() => expect(view.result.current).toBe("indexing"));
    await waitFor(() => expect(ownership.signals).toHaveLength(1));
    expect(ownership.signals[0]!.aborted).toBe(false);

    view.unmount();
    expect(ownership.signals[0]!.aborted).toBe(true);
    await new Promise((settled) => setTimeout(settled, 30));
    expect(ownership.signals).toHaveLength(1);
  });
});
