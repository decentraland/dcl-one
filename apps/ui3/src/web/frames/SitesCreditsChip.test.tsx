import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { createShopClient } from "../../shop/api";
import { ShopServicesProvider } from "../../shop/checkout/services";
import { catalogTransport } from "../../shop/discovery/fixtures";
import SitesCreditsChip from "./SitesCreditsChip";

const ACCOUNT = "0x0000000000000000000000000000000000000b0b";
const BALANCE = { credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents: 123_450, credits: 12_345 } };

function chip(answers: Record<string, unknown>) {
  const transport = catalogTransport("ok", answers);
  const signer = { address: ACCOUNT, sign: async () => ({}) };
  const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: async () => signer });
  return render(
    <MemoryRouter>
      <ShopServicesProvider value={{ client }}>
        <SitesCreditsChip account={ACCOUNT} />
      </ShopServicesProvider>
    </MemoryRouter>,
  );
}

describe("SitesCreditsChip", () => {
  it("links the balance to the credits page with the credits mark, without the shop's test id", async () => {
    chip({ [`/v1/users/${ACCOUNT}/credits`]: BALANCE });
    const link = await screen.findByRole("link", { name: "12,345 credits" });
    expect(link).toHaveTextContent("12,345");
    expect(link.getAttribute("href")).toBe("/shop/credits");
    expect(link.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryByTestId("navbar-credits")).toBeNull();
  });

  it("shows nothing while the balance is unknown", async () => {
    const { container } = chip({});
    await waitFor(() => expect(container.querySelector("a")).toBeNull());
    expect(screen.queryByRole("link")).toBeNull();
  });
});
