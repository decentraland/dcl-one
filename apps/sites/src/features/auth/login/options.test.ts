import { describe, expect, it } from "vitest";

import { layoutOffers, layoutOptions, optionSetFor, requestedOption, signInOptionsMode } from "./options";

const DESKTOP = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";
const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148";
const ANDROID = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/131.0 Mobile Safari/537.36";

describe("the sign-in layout", () => {
  it("reads the rollout's variant as upstream does", () => {
    expect(signInOptionsMode(null)).toBe("full");
    expect(signInOptionsMode("")).toBe("full");
    expect(signInOptionsMode("two-options")).toBe("two");
    expect(signInOptionsMode("one-option")).toBe("one");
    expect(signInOptionsMode("anything-else")).toBe("one");
  });

  it("offers decentraland.org's live layout: Google alone, or Google and MetaMask with a wallet", () => {
    const set = optionSetFor(DESKTOP);
    expect(layoutOptions(set, "two", false)).toEqual({
      first: "google",
      more: ["metamask", "discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
    });
    expect(layoutOptions(set, "two", true)).toEqual({
      first: "google",
      second: "metamask",
      more: ["discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
    });
  });

  it("puts MetaMask and Google first without the rollout, and leaves the email box out of the buttons", () => {
    expect(layoutOptions(optionSetFor(DESKTOP), "full", false)).toEqual({
      first: "metamask",
      second: "google",
      more: ["discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
    });
  });

  it("shows MetaMask alone in the one-option layout, or every option behind More Options without a wallet", () => {
    const set = optionSetFor(DESKTOP);
    expect(layoutOptions(set, "one", true)).toEqual({
      first: "metamask",
      more: ["google", "discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
    });
    expect(layoutOptions(set, "one", false)).toEqual({
      more: ["google", "metamask", "discord", "apple", "x", "fortmatic", "coinbase", "wallet-connect"],
    });
  });

  it("gives phones their platform's sign-in first and no extension-only wallets", () => {
    expect(layoutOptions(optionSetFor(IPHONE), "full", false)).toEqual({ first: "apple", second: "google", more: ["metamask", "wallet-connect"] });
    expect(layoutOptions(optionSetFor(ANDROID), "full", false)).toEqual({ first: "google", second: "apple", more: ["metamask", "wallet-connect"] });
    // iPadOS asks for the desktop site; its touch points still give it away.
    expect(optionSetFor(DESKTOP, 5).primary).toBe("apple");
    expect(optionSetFor(DESKTOP, 0).primary).toBe("email");
  });
});

describe("a method the Explorer requests", () => {
  it("is one of the options or nothing", () => {
    expect(requestedOption("google")).toBe("google");
    expect(requestedOption("wallet-connect")).toBe("wallet-connect");
    expect(requestedOption("toString")).toBeNull();
    expect(requestedOption("javascript:alert(1)")).toBeNull();
    expect(requestedOption(null)).toBeNull();
  });

  it("starts only where the layout offers it", () => {
    const layout = layoutOptions(optionSetFor(DESKTOP), signInOptionsMode("two-options"), false);
    expect(layoutOffers(layout, "google")).toBe(true);
    expect(layoutOffers(layout, "discord")).toBe(true);
    expect(layoutOffers(layout, "email")).toBe(false);
  });
});

