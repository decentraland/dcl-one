import { describe, expect, test, vi } from "vitest";
import { fireEvent, render as rtlRender, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";

import PlaceDetail from "./PlaceDetail";

function render(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const place = {
  id: "p1",
  title: "Mana Mania Raffle",
  coords: "-13,1",
  parcels: 1,
  favorites: 2,
  views: 0,
  approval: 0,
  creator: "metaskins studios",
  creatorAddress: "0x296fa4213a6a88590328008dd5f5b535cf37bd66",
  categories: ["game", "poi"],
  updated: "5/9/2023",
  description: "My new Decentraland project",
  hue: 120,
};

describe("PlaceDetail", () => {
  test("hides the visit and like counters while both are zero, and has no Home button", () => {
    render(<PlaceDetail place={place} />);
    expect(screen.queryByTitle("Visits")).toBeNull();
    expect(screen.queryByTitle("Likes")).toBeNull();
    expect(screen.queryByRole("button", { name: "Home" })).toBeNull();
    expect(screen.getByRole("button", { name: "Like" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Share" })).toBeInTheDocument();
  });

  test("shows the counters once either is non-zero", () => {
    render(<PlaceDetail place={{ ...place, views: 12 }} />);
    expect(screen.getByTitle("Visits")).toHaveTextContent("12");
    expect(screen.getByTitle("Likes")).toHaveTextContent("0%");
  });

  test("the creator is a profile button when the catalog has an address, plain text otherwise", () => {
    const { unmount } = render(<PlaceDetail place={place} />);
    const creator = screen.getByRole("button", { name: "metaskins studios" });
    expect(creator).toHaveAttribute("title", place.creatorAddress);
    unmount();
    render(<PlaceDetail place={{ ...place, creatorAddress: null }} />);
    expect(screen.queryByRole("button", { name: "metaskins studios" })).toBeNull();
    expect(screen.getByText("metaskins studios")).toBeInTheDocument();
  });

  test("categories show as tags", () => {
    render(<PlaceDetail place={place} />);
    const tags = screen.getByRole("list", { name: "Categories" });
    expect(within(tags).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["Game", "Point of interest"]);
  });

  test("a world names its realm", () => {
    render(<PlaceDetail place={{ ...place, world: true, worldName: "metaskins.dcl.eth" }} />);
    expect(screen.getByText("metaskins.dcl.eth")).toBeInTheDocument();
  });

  test("favorite and like save through the host's writes with the place id", async () => {
    const onLike = vi.fn(async () => true);
    const onFavorite = vi.fn(async () => true);
    render(<PlaceDetail place={place} onLike={onLike} onFavorite={onFavorite} />);

    fireEvent.click(screen.getByRole("button", { name: "Favorite" }));
    expect(onFavorite).toHaveBeenLastCalledWith("p1", true);
    fireEvent.click(screen.getByRole("button", { name: "Like" }));
    expect(onLike).toHaveBeenLastCalledWith("p1", true);
    fireEvent.click(screen.getByRole("button", { name: "Like" }));
    expect(onLike).toHaveBeenLastCalledWith("p1", null);
    fireEvent.click(screen.getByRole("button", { name: "Dislike" }));
    expect(onLike).toHaveBeenLastCalledWith("p1", false);

    await waitFor(() => expect(screen.getByRole("button", { name: "Favorite" })).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByRole("button", { name: "Dislike" })).toHaveAttribute("aria-pressed", "true");
  });

  test("a write that fails puts the button back", async () => {
    const onFavorite = vi.fn(() => Promise.reject(new Error("Sign in to save places.")));
    render(<PlaceDetail place={place} onFavorite={onFavorite} />);
    const button = screen.getByRole("button", { name: "Favorite" });
    fireEvent.click(button);
    expect(button).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(button).toHaveAttribute("aria-pressed", "false"));
  });
});
