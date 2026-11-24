import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import ShopCreditsChip from "./ShopCreditsChip";

const { balance } = vi.hoisted(() => ({ balance: vi.fn() }));
vi.mock("../../credits/hooks", () => ({ useBalance: balance }));
vi.mock("../../wallet/context", () => ({ useShopWallet: () => ({ account: "account" }) }));

describe("ShopCreditsChip", () => {
  beforeEach(() => { balance.mockReset(); });
  afterEach(() => window.history.replaceState({}, "", "/"));

  describe("when the balance is unavailable", () => {
    it.each([{ isLoading: true }, { isError: true }])("should hide the balance for %o", (result) => {
      balance.mockReturnValue(result);
      render(<MemoryRouter><ShopCreditsChip /></MemoryRouter>);
      expect(screen.queryByLabelText(/shop credits/i)).toBeNull();
    });
  });

  describe("when a zero balance is loaded in the in-app purchase view", () => {
    beforeEach(() => {
      window.history.replaceState({}, "", "/?view=mobile-iap");
      balance.mockReturnValue({ data: { credits: 0 } });
      render(<MemoryRouter><ShopCreditsChip /></MemoryRouter>);
    });
    it("should show the amount without a purchase link", () => {
      expect(screen.getByLabelText("0 shop credits")).toHaveTextContent("0");
      expect(screen.queryByRole("link")).toBeNull();
    });
  });
});
