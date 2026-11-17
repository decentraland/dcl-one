import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import StProfileCommunitiesTab from "./StProfileCommunitiesTab";

describe("StProfileCommunitiesTab", () => {
  it("links each community card to its /communities page", () => {
    render(
      <StProfileCommunitiesTab
        chrome={false}
        isOwnProfile
        communities={[{ id: "c 1", name: "Builders", membersCount: 3, role: "member", thumb: null }]}
      />,
    );
    expect(screen.getByText("Builders").closest("a")).toHaveAttribute("href", "/communities/c%201");
  });
});
