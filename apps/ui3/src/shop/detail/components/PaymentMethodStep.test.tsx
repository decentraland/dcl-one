// The method step on its own: which rows are ticked as the rails resolve and change, what a tick that cannot pay says,
// and what a disabled row reads out.
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { manaWei, usdCents, ZERO_MANA } from "@shop/model/money";
import type { CartLineDisplay } from "@shop/model/purchase";
import PaymentMethodStep, { type StepRail, type StepRails } from "./PaymentMethodStep";

const display = { name: "Rainbow Hat", rarity: "epic", category: "wearable" } as CartLineDisplay;
const CREDITS: StepRails["credits"] = { credits: 12 };
const MANA: StepRails["mana"] = { manaWei: manaWei(23n * 10n ** 17n) };
const SPLIT: StepRails["combined"] = { creditsCents: usdCents(30), credits: 3, manaWei: manaWei(17n * 10n ** 17n) };
const NONE: StepRails = { credits: null, mana: null, combined: null };

function step(rails: StepRails, extra: { initial?: StepRail | null; balanceCredits?: number | null } = {}) {
  const picks: (StepRail | null)[] = [];
  const props = {
    display,
    priceCredits: 12,
    priceManaWei: manaWei(23n * 10n ** 17n),
    balanceCredits: extra.balanceCredits === undefined ? 3 : extra.balanceCredits,
    manaBalanceWei: ZERO_MANA,
    onPick: (r: StepRail | null) => void picks.push(r),
    ready: true,
    onBuy: () => {},
    onClose: () => {},
    initial: extra.initial,
  };
  const view = render(<PaymentMethodStep {...props} rails={rails} />);
  return { picks, rerender: (next: StepRails, more: Partial<typeof props> = {}) => view.rerender(<PaymentMethodStep {...props} {...more} rails={next} />) };
}

const credits = () => screen.getByRole("checkbox", { name: /^Credits/ });
const mana = () => screen.getByRole("checkbox", { name: /^Polygon MANA/ });
const buy = () => screen.getByRole("button", { name: "Buy" });

describe("the payment method step", () => {
  it("nothing payable yet, then the Credits land: the Credits are ticked once they pay", () => {
    const { picks, rerender } = step(NONE);
    expect(credits()).toHaveAttribute("aria-checked", "false");
    expect(buy()).toBeDisabled();
    rerender({ ...NONE, credits: CREDITS });
    expect(credits()).toHaveAttribute("aria-checked", "true");
    expect(picks.at(-1)).toBe("credits");
  });

  it("a ticked rail that stops paying is dropped, and the one that still pays is ticked instead", () => {
    const { picks, rerender } = step({ ...NONE, credits: CREDITS, mana: MANA });
    expect(credits()).toHaveAttribute("aria-checked", "true");
    rerender({ ...NONE, mana: MANA });
    expect(credits()).toHaveAttribute("aria-checked", "false");
    expect(mana()).toHaveAttribute("aria-checked", "true");
    expect(picks.at(-1)).toBe("mana");
  });

  it("one tick that cannot pay alone while both together would says why, over a disabled Buy", () => {
    step({ ...NONE, combined: SPLIT });
    expect(credits()).toHaveAttribute("aria-checked", "true");
    expect(mana()).toHaveAttribute("aria-checked", "true");
    expect(screen.queryByTestId("payment-method-alone")).toBeNull();
    fireEvent.click(mana());
    expect(screen.getByTestId("payment-method-alone")).toHaveTextContent("Your Credits don't cover this on their own. Tick MANA too to pay the rest with it.");
    expect(buy()).toBeDisabled();
    fireEvent.click(mana());
    expect(screen.queryByTestId("payment-method-alone")).toBeNull();
    expect(buy()).toBeEnabled();
  });

  it("a Credits row that cannot pay is reachable and reads out why, once", () => {
    step({ ...NONE, mana: MANA }, { balanceCredits: 0 });
    expect(credits()).toHaveAttribute("aria-disabled", "true");
    expect(credits()).not.toBeDisabled();
    expect(credits()).toHaveAccessibleDescription("Not enough Credits");
    // Its description, not also part of its name.
    expect(credits()).not.toHaveAccessibleName(/Not enough Credits/);
    fireEvent.click(credits());
    expect(credits()).toHaveAttribute("aria-checked", "false");
  });

  it("says nothing of 'not enough' when the balance could not be read", () => {
    step({ ...NONE, mana: MANA }, { balanceCredits: null });
    expect(credits()).toHaveAttribute("aria-disabled", "true");
    expect(credits()).not.toHaveAccessibleDescription(/Not enough Credits/);
  });

  it("a MANA row that cannot pay carries its reason in its name, read once", () => {
    step({ ...NONE, credits: CREDITS });
    expect(mana()).toHaveAttribute("aria-disabled", "true");
    expect(mana()).toHaveAccessibleName(/Not enough MANA/);
    expect(mana()).not.toHaveAccessibleDescription(/Not enough MANA/);
  });

  it("drawn again after a failure, the buyer's earlier pick is ticked while it still pays", () => {
    step({ ...NONE, credits: CREDITS, mana: MANA }, { initial: "mana" });
    expect(mana()).toHaveAttribute("aria-checked", "true");
    expect(credits()).toHaveAttribute("aria-checked", "false");
  });

  it("the earlier pick also holds when the step mounts before its rails are priced again", () => {
    const { picks, rerender } = step(NONE, { initial: "mana" });
    rerender({ ...NONE, credits: CREDITS, mana: MANA });
    expect(mana()).toHaveAttribute("aria-checked", "true");
    expect(credits()).toHaveAttribute("aria-checked", "false");
    expect(picks.at(-1)).toBe("mana");
  });

  it("a Credits balance that could not be read shows a dash, not 0", () => {
    step({ ...NONE, mana: MANA }, { balanceCredits: null });
    expect(credits()).toHaveTextContent("Credits Balance:—");
  });
});
