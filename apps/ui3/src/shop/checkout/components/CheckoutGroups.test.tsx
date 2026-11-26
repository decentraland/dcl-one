import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { GroupProgress } from "@shop/model/purchase";
import CheckoutGroups from "./CheckoutGroups";

const progress = (keyId: string, status: GroupProgress["status"]): GroupProgress =>
  ({ keyId, key: { method: "credits", chainId: 137, contract: "0x" + "ab".repeat(20), call: { kind: "storeBuy" } }, lineIds: [], status, creditIds: [], txHash: null, failure: null, updatedAt: 0, attempts: 1 }) as unknown as GroupProgress;

const groups = (held: GroupProgress["status"], other: GroupProgress["status"]) => (
  <CheckoutGroups groups={[]} lines={[]} progress={[progress("held", held), progress("other", other)]} results={{}} walletNeededKeys={[]} />
);

describe("the checkout's payments", () => {
  it("says a payment waits while another of the checkout is in flight, wherever it is listed, and is ready once that one settles", () => {
    // Listed before the one in flight, with no result of its own (a reloaded page).
    const view = render(groups("idle", "pending"));
    const rows = () => screen.getAllByTestId("checkout-group");
    expect(rows().map((r) => r.getAttribute("data-tone"))).toEqual(["wait", "busy"]);
    expect(rows()[0]!.textContent).toContain("Waiting for another payment in this checkout to be confirmed.");
    // A hand-off not yet followed to a hash holds it too.
    view.rerender(groups("idle", "submitted"));
    expect(rows()[0]!.getAttribute("data-tone")).toBe("wait");
    // The other one settled: it is ready again.
    view.rerender(groups("idle", "confirmed"));
    expect(rows().map((r) => r.getAttribute("data-tone"))).toEqual(["idle", "done"]);
    expect(rows()[0]!.textContent).toContain("Ready to confirm");
  });
});
