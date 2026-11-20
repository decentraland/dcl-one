import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { createDemo, DemoShell } from "../checkout/demo";
import { useSettlement } from "./useSettlement";

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
});
