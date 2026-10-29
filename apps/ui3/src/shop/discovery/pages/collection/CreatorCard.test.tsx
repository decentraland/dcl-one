// from: shop@2d3bf62:app/src/lib/avatarColor.spec.ts (the display-name vectors)
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { getDisplayName } from "../../lib/avatarColor";
import CreatorCard from "./CreatorCard";

describe("the avatar backdrop", () => {
  it("builds the in-world display name", () => {
    expect(getDisplayName("Alice", true, "0x1234abcd")).toBe("Alice");
    expect(getDisplayName("Alice", false, "0x1234abcd")).toBe("Alice#abcd");
    expect(getDisplayName("A🚀l-i.c e", true, null)).toBe("Alice");
    expect(getDisplayName("🚀🚀", false, "0x1234abcd")).toBe("");
  });
});

describe("the collection creator card", () => {
  it("shows the short address, a copy control and a profile link", () => {
    const address = "0x7e56aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa374c";
    render(<CreatorCard address={address} />);
    expect(screen.getByTestId("creator-card-copy")).toHaveAccessibleName("Copy account");
    expect(screen.getByTestId("creator-card-view")).toHaveAttribute("href", `/profile/${address}`);
  });
});
