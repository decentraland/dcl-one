import type { ReactElement, ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render as renderDom, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import PlaceDetail from "./PlaceDetail";
import { pressedPlaceButtons as pressed } from "../../test/placeButtons";

const client = new QueryClient();
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
const render = (ui: ReactElement) => renderDom(ui, { wrapper });

const place = { id: "plaza", title: "Genesis Plaza", coords: "-3,-2", parcels: 1, favorites: 135, views: 0, approval: 0, creator: "DAO", updated: "", description: "", hue: 0 };

test("the account's saved state, known only after the first render, presses its buttons", () => {
  const { rerender } = render(<PlaceDetail place={place} />);
  expect(pressed()).toEqual(["false", "false", "false"]);
  rerender(<PlaceDetail place={place} viewer={{ liked: true, favorite: true }} />);
  expect(pressed()).toEqual(["true", "false", "true"]);
  rerender(<PlaceDetail place={place} viewer={{ liked: false, favorite: false }} />);
  expect(pressed()).toEqual(["false", "true", "false"]);
});

test("a saved state known at the first render is shown at once, and an unknown one changes nothing", () => {
  const { rerender } = render(<PlaceDetail place={place} viewer={{ liked: null, favorite: true }} />);
  expect(pressed()).toEqual(["false", "false", "true"]);
  rerender(<PlaceDetail place={place} viewer={null} />);
  expect(pressed()).toEqual(["false", "false", "true"]);
});

test("a button the visitor pressed before the saved state arrived keeps their choice; the others follow the account", async () => {
  const user = userEvent.setup();
  const onFavorite = vi.fn(() => Promise.resolve());
  const onLike = vi.fn(() => Promise.resolve());
  const { rerender } = render(<PlaceDetail place={place} onFavorite={onFavorite} onLike={onLike} />);
  await user.click(screen.getByRole("button", { name: "Favorite" }));
  expect(onFavorite).toHaveBeenCalledWith("plaza", true);
  rerender(<PlaceDetail place={place} viewer={{ liked: true, favorite: false }} onFavorite={onFavorite} onLike={onLike} />);
  expect(pressed()).toEqual(["true", "false", "true"]);
  expect(onLike).not.toHaveBeenCalled();
});
