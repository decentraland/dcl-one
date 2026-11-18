// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import type { SubscriptionDetails } from "@/data/lib/account/notifications";

import { NotificationGroupAccordion } from "./NotificationGroupAccordion";

const details: SubscriptionDetails = {
  ignore_all_email: false,
  ignore_all_in_app: false,
  message_type: { item_sold: { email: true, in_app: true } },
};

function show(disabled = false) {
  const onToggleType = vi.fn();
  render(
    <NotificationGroupAccordion
      group="marketplace"
      details={details}
      disabled={disabled}
      onToggleType={onToggleType}
    />,
  );
  return { onToggleType, header: screen.getByRole("button", { name: "Marketplace" }) };
}

afterEach(cleanup);

describe("NotificationGroupAccordion", () => {
  it("starts closed, its types out of reach, and opens and closes from its header", async () => {
    const user = userEvent.setup();
    const { header } = show();

    expect(header.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("switch")).toBeNull();
    const region = document.getElementById(header.getAttribute("aria-controls")!)!;
    expect(region.hidden).toBe(true);

    await user.click(header);
    expect(header.getAttribute("aria-expanded")).toBe("true");
    expect(region.hidden).toBe(false);
    expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);

    await user.click(header);
    expect(header.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("opens from the keyboard and reports a toggled type", async () => {
    const user = userEvent.setup();
    const { header, onToggleType } = show();

    header.focus();
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("switch", { name: "Bid Received" }));

    expect(onToggleType).toHaveBeenCalledWith("bid_received", true);
  });

  it("keeps its types locked when disabled, so nothing is reported", async () => {
    const user = userEvent.setup();
    const { header, onToggleType } = show(true);

    await user.click(header);
    const item = screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement;
    expect(item.disabled).toBe(true);
    await user.click(item);

    expect(onToggleType).not.toHaveBeenCalled();
    expect(item.checked).toBe(true);
  });
});
