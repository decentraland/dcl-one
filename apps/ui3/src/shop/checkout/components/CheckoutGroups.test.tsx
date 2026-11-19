import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { GroupProgress } from "@shop/model/purchase";
import { AFTER_PENDING } from "../purchase/executor";
import CheckoutGroups from "./CheckoutGroups";

const progress = (keyId: string, status: GroupProgress["status"]): GroupProgress =>
  ({ keyId, key: { method: "credits", chainId: 137, contract: "0x" + "ab".repeat(20), call: { kind: "storeBuy" } }, lineIds: [], status, creditIds: [], txHash: null, failure: null, updatedAt: 0, attempts: 1 }) as unknown as GroupProgress;

describe("the checkout's payments", () => {
  it("says a payment held behind one still being confirmed waits for it, and one never held is ready", () => {
    render(
      <CheckoutGroups
        groups={[]}
        lines={[]}
        progress={[progress("first", "pending"), progress("held", "idle"), progress("ready", "idle")]}
        results={{ first: { status: "pending", txHash: null }, held: { status: "skipped", reason: AFTER_PENDING } }}
        walletNeededKeys={[]}
      />,
    );
    const rows = screen.getAllByTestId("checkout-group");
    expect(rows.map((r) => r.getAttribute("data-tone"))).toEqual(["busy", "wait", "idle"]);
    expect(rows[1]!.textContent).toContain("Waits for the part above to be confirmed. Confirm again once it is.");
    expect(rows[2]!.textContent).toContain("Ready to confirm");
  });
});
