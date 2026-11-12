import type { ComponentProps, ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render as renderDom, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import PlaceDetail from "./PlaceDetail";

vi.mock("../../data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: vi.fn(async () => ({})) }));

type Props = ComponentProps<typeof PlaceDetail>;
const place = { id: "plaza", title: "Genesis Plaza", coords: "-3,-2", description: "", approval: 87, favorites: 1135 };
const saved = () => Promise.resolve();
const refused = () => Promise.reject(new Error("Sign in to rate or save places."));

const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
const page = (over: Partial<Props> = {}) => <PlaceDetail place={place} livePlayers={3} onLike={saved} onFavorite={saved} {...over} />;
const mount = (over: Partial<Props> = {}) => renderDom(page(over), { wrapper });
const button = (name: string) => screen.getByRole("button", { name });
const pressed = () => ["Like", "Dislike", "Favorite"].map((name) => button(name).getAttribute("aria-pressed"));

test("the live view and the jump-in card both carry the three buttons and the place's counts", () => {
  const { unmount } = mount();
  const live = screen.getByRole("group", { name: "Rate and save this place" });
  expect(within(live).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["Like", "Dislike", "Favorite"]);
  expect(live).toHaveTextContent("87% likes");
  expect(live).toHaveTextContent("1,135 favorites");
  unmount();
  mount({ livePlayers: 0 });
  const card = within(screen.getByRole("dialog", { name: "Genesis Plaza" })).getByRole("group", { name: "Rate and save this place" });
  expect(within(card).getAllByRole("button")).toHaveLength(3);
  expect(card).toHaveTextContent("1,135 favorites");
});

test("a place nobody rated shows no approval, and one save is singular", () => {
  mount({ place: { ...place, approval: null, favorites: 1 } });
  const group = screen.getByRole("group", { name: "Rate and save this place" });
  expect(group).not.toHaveTextContent("%");
  expect(group).toHaveTextContent(/1 favorite$/);
});

test("without the host's writes there are no buttons", () => {
  mount({ onLike: undefined, onFavorite: undefined });
  expect(screen.queryByRole("button", { name: "Like" })).toBeNull();
  expect(screen.queryByRole("group", { name: "Rate and save this place" })).toBeNull();
});

test("like and dislike exclude each other, and a second press clears the choice", async () => {
  const user = userEvent.setup();
  const onLike = vi.fn(saved);
  mount({ onLike });
  await user.click(button("Like"));
  expect(onLike).toHaveBeenLastCalledWith("plaza", true);
  expect(pressed()).toEqual(["true", "false", "false"]);
  await user.click(button("Dislike"));
  expect(onLike).toHaveBeenLastCalledWith("plaza", false);
  expect(pressed()).toEqual(["false", "true", "false"]);
  await user.click(button("Dislike"));
  expect(onLike).toHaveBeenLastCalledWith("plaza", null);
  expect(pressed()).toEqual(["false", "false", "false"]);
});

test("favorite saves and removes the save", async () => {
  const user = userEvent.setup();
  const onFavorite = vi.fn(saved);
  mount({ onFavorite });
  await user.click(button("Favorite"));
  expect(onFavorite).toHaveBeenLastCalledWith("plaza", true);
  expect(pressed()).toEqual(["false", "false", "true"]);
  await user.click(button("Favorite"));
  expect(onFavorite).toHaveBeenLastCalledWith("plaza", false);
  expect(pressed()).toEqual(["false", "false", "false"]);
});

test("signed out, the host refuses the write and each button goes back", async () => {
  const user = userEvent.setup();
  const onLike = vi.fn(refused);
  const onFavorite = vi.fn(refused);
  mount({ onLike, onFavorite });
  for (const name of ["Like", "Dislike", "Favorite"]) {
    await user.click(button(name));
    await waitFor(() => expect(pressed()).toEqual(["false", "false", "false"]));
  }
  expect(onLike).toHaveBeenCalledTimes(2);
  expect(onFavorite).toHaveBeenCalledTimes(1);
});

test("a write that fails puts the button back to the account's saved state", async () => {
  const user = userEvent.setup();
  let fail!: (reason: Error) => void;
  const onLike = vi.fn(() => new Promise<void>((_, reject) => { fail = reject; }));
  const onFavorite = vi.fn(() => Promise.reject(new Error("503")));
  mount({ viewer: { liked: true, favorite: true }, account: "0xa", onLike, onFavorite });
  await user.click(button("Dislike"));
  // The press shows while the write is on its way.
  expect(pressed()).toEqual(["false", "true", "true"]);
  fail(new Error("503"));
  await waitFor(() => expect(pressed()).toEqual(["true", "false", "true"]));
  await user.click(button("Favorite"));
  expect(onFavorite).toHaveBeenLastCalledWith("plaza", false);
  await waitFor(() => expect(pressed()).toEqual(["true", "false", "true"]));
});

test("the account's saved state, known only after the first render, presses its buttons", () => {
  const { rerender } = mount({ account: "0xa" });
  expect(pressed()).toEqual(["false", "false", "false"]);
  rerender(page({ account: "0xa", viewer: { liked: true, favorite: true } }));
  expect(pressed()).toEqual(["true", "false", "true"]);
  rerender(page({ account: "0xa", viewer: { liked: false, favorite: false } }));
  expect(pressed()).toEqual(["false", "true", "false"]);
});

test("a button the visitor pressed before the saved state arrived keeps their choice; the others follow the account", async () => {
  const user = userEvent.setup();
  const onLike = vi.fn(saved);
  const { rerender } = mount({ account: "0xa", onLike });
  await user.click(button("Favorite"));
  rerender(page({ account: "0xa", onLike, viewer: { liked: true, favorite: false } }));
  expect(pressed()).toEqual(["true", "false", "true"]);
  expect(onLike).not.toHaveBeenCalled();
});

test("a press stays when the page changes between the live view and the jump-in card", async () => {
  const user = userEvent.setup();
  const { rerender } = mount();
  await user.click(button("Like"));
  rerender(page({ livePlayers: 0 }));
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(pressed()).toEqual(["true", "false", "false"]);
});

test("a choice made under one account is forgotten when the account changes", async () => {
  const user = userEvent.setup();
  const { rerender } = mount({ account: "0xa", viewer: { liked: null, favorite: false } });
  await user.click(button("Like"));
  await user.click(button("Favorite"));
  expect(pressed()).toEqual(["true", "false", "true"]);
  rerender(page({ account: null, viewer: null }));
  expect(pressed()).toEqual(["false", "false", "false"]);
  rerender(page({ account: "0xb", viewer: { liked: false, favorite: false } }));
  expect(pressed()).toEqual(["false", "true", "false"]);
});
