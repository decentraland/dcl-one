import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import ShopFooter from "./ShopFooter";

describe("ShopFooter", () => {
  let request: ReturnType<typeof vi.fn>;
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    request = vi.fn();
    vi.stubGlobal("fetch", request);
    user = userEvent.setup();
    render(<ShopFooter />);
  });

  afterEach(() => vi.unstubAllGlobals());

  it("should keep external resources in a separate browsing context", () => {
    const link = screen.getAllByRole("link", { name: "Vote" })[0]!;
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel")).toContain("noopener");
  });

  it("should expand one resource group at a time", async () => {
    const start = screen.getByRole("button", { name: "Getting Started", hidden: true });
    await user.click(start);
    expect(start.getAttribute("aria-expanded")).toBe("true");
    await user.click(screen.getByRole("button", { name: "Resources", hidden: true }));
    expect(start.getAttribute("aria-expanded")).toBe("false");
  });

  it("should reject an invalid email before submitting", async () => {
    await user.click(screen.getByRole("button", { name: "Subscribe" }));
    expect(screen.getByRole("status").textContent).toBe("Enter a valid email address.");
    expect(request).not.toHaveBeenCalled();
  });

  it("should preserve the address after a failed request and submit it on retry", async () => {
    request.mockResolvedValueOnce(new Response(null, { status: 503 })).mockResolvedValueOnce(new Response(null));
    await user.type(screen.getByRole("textbox", { name: "Email address" }), "buyer@example.com");
    const submit = screen.getByRole("button", { name: "Subscribe" });
    await user.click(submit);
    expect(await screen.findByText("Something went wrong. Please try again.")).toBeTruthy();
    await user.click(submit);
    expect(await screen.findByText("Thanks! You're subscribed.")).toBeTruthy();
    expect(request).toHaveBeenLastCalledWith("/v1/newsletter", expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ email: "buyer@example.com", source: "Shop" }),
    }));
  });
});
