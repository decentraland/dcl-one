import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

const profile = vi.hoisted(() => ({ state: { profile: null as null | { name: string; picture: string | null; claimed: boolean | null }, loading: true } }));
vi.mock("../../discovery/hooks", () => ({ useCreatorProfile: () => profile.state }));

import { BadgeSkeleton, DetailCreator } from "./DetailBadges";

const CREATOR = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

afterEach(() => {
  profile.state = { profile: null, loading: true };
});

describe("the item page's creator badge", () => {
  it("holds its place with a skeleton while the profile is on its way, instead of showing the address", () => {
    render(
      <MemoryRouter>
        <DetailCreator address={CREATOR} />
      </MemoryRouter>,
    );
    expect(screen.getByTestId("creator-loading")).toBeTruthy();
    expect(screen.queryByTestId("creator-display")).toBeNull();
  });

  it("shows the creator's name once the profile lands", () => {
    profile.state = { profile: { name: "doki3D", picture: null, claimed: true }, loading: false };
    render(
      <MemoryRouter>
        <DetailCreator address={CREATOR} />
      </MemoryRouter>,
    );
    expect(screen.queryByTestId("creator-loading")).toBeNull();
    expect(screen.getByTestId("creator-display").textContent).toBe("Doki3D");
  });

  it("keeps the cart's small badge as it was: no skeleton there", () => {
    render(
      <MemoryRouter>
        <DetailCreator address={CREATOR} size="sm" />
      </MemoryRouter>,
    );
    expect(screen.queryByTestId("creator-loading")).toBeNull();
    expect(screen.getByTestId("creator-display")).toBeTruthy();
  });

  it("draws the skeleton as a 48px circle and a name bar, hidden from assistive tech", () => {
    const { container } = render(<BadgeSkeleton testId="collection-loading" />);
    const sk = screen.getByTestId("collection-loading");
    expect(sk.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelectorAll("span")).toHaveLength(2);
  });
});
