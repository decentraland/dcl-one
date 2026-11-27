import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { BOOK } from "@shop/payment/fixtures";
import { nftRow } from "../../market/fixtures";
import { fakeWallet } from "../../market/testing";
import { mountShop } from "../../market/testHarness";

describe("when an owner confirms a listing price", () => {
  it("returns focus to the listing button and clears the confirmation after Escape", async () => {
    const user = userEvent.setup();
    const wallet = fakeWallet();
    mountShop({
      path: `/shop/token/${BOOK.collection}/105313/sell`,
      who: "web3",
      wallet,
      routes: [
        {
          method: "GET",
          path: /^\/v1\/nfts$/,
          reply: () => ({ body: { data: [nftRow({ tokenId: "105313", itemId: "8", name: "Cyber Boots", owner: wallet.address })], total: 1 } }),
        },
      ],
    });
    await user.type(await screen.findByLabelText("Price"), "7.25");
    const opener = screen.getByRole("button", { name: "List for sale" });
    await user.click(opener);
    const dialog = await screen.findByRole("dialog", { name: "Please confirm" });
    const confirm = within(dialog).getByRole("textbox", { name: "Confirm the price" });
    expect(confirm).toHaveFocus();
    await user.type(confirm, "7.25");
    await user.keyboard("{Escape}");
    expect(dialog).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
    await user.click(opener);
    const next = within(await screen.findByRole("dialog", { name: "Please confirm" }));
    expect(next.getByRole("textbox", { name: "Confirm the price" })).toHaveValue("");
    expect(next.getByRole("button", { name: "Proceed" })).toBeDisabled();
  });
});
