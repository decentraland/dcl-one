import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { BOOK } from "@shop/payment/fixtures";
import { GaslessUnavailableError } from "../chain/metatx";
import { requireContracts } from "../chain/contracts";
import { AuthorizationKind, type ShopAuthorization } from "../lib/authorizations";
import AuthorizeStep from "./AuthorizeStep";

describe("when the approval's grant is refused by the relay's request limit", () => {
  let auth: ShopAuthorization;
  let grant: () => Promise<void>;
  let onAuthorized: () => void;

  beforeEach(() => {
    auth = { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(137).mana.address, spenderAddress: BOOK.marketplace.contract, chainId: 137 };
    grant = vi.fn(async () => {
      throw new GaslessUnavailableError("Too many requests", "relayer-rejected", { error: "api_limit_reached" });
    });
    onAuthorized = vi.fn();
    render(<AuthorizeStep auth={auth} grant={grant} onAuthorized={onAuthorized} onCancel={vi.fn()} portal={false} />);
  });

  it("should tell the buyer to wait and keep the step open to try again", async () => {
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Too many requests from your connection. Wait a few seconds and try again.");
    expect(screen.getByRole("button", { name: "Approve" })).toBeEnabled();
    expect(onAuthorized).not.toHaveBeenCalled();
  });
});
