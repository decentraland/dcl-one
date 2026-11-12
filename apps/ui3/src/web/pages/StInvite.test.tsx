import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import StInvite, { referralUrl } from "./StInvite";

describe("referralUrl", () => {
  it("puts the referrer on the sign-in and on its redirect to the download", () => {
    const url = new URL(referralUrl("0xabc"), "https://example.org");
    expect(url.pathname).toBe("/auth/login");
    expect(url.searchParams.get("referrer")).toBe("0xabc");
    expect(url.searchParams.get("redirectTo")).toBe("/download?referrer=0xabc");
  });

  it("sends a visit without a referrer to the download after the sign-in", () => {
    const url = new URL(referralUrl(""), "https://example.org");
    expect(url.searchParams.has("referrer")).toBe(false);
    expect(url.searchParams.get("redirectTo")).toBe("/download");
  });
});

describe("StInvite", () => {
  it("links both heroes' CTAs to the referral sign-in", () => {
    render(<StInvite referrer={{ name: "bevynaut2", ethAddress: "0xABC" }} />);
    const expected = referralUrl("0xabc");
    expect(screen.getByRole("link", { name: "JOIN NOW" })).toHaveAttribute("href", expected);
    expect(screen.getByRole("link", { name: "JOIN FREE" })).toHaveAttribute("href", expected);
  });
});
