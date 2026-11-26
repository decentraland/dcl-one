// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { openSignIn } from "@/features/components/auth/signin-store";
import PlaceDetailRoute from "./places_.$id";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/features/explorer/scene-chat/useSceneChat", () => ({ useSceneChat: () => ({ status: "loading", messages: [] }) }));
vi.mock("@/data/lib/catalyst/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/catalyst/client")>()),
  postJSON: vi.fn(async () => ({ ok: true })),
  signedGetJSON: vi.fn(async () => ({ ok: true, data: { user_like: false, user_dislike: false, user_favorite: true } })),
}));
const auth = vi.hoisted(() => ({ identity: null as unknown, isConnected: false }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));

const IDENTITY: AuthIdentity = {
  signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295",
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
const PLACE_ID = "66ef7bd9-3060-4e1a-a6d7-edc8d251e0cb";
const PLACE = { id: PLACE_ID, title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false, world_name: null, contact_name: "Studio", owner: null, like_rate: 0.874, favorites: 135 };

function mount(livePlayers = 0) {
  const loaderData = { place: PLACE, scenes: [], sid: "sid-1", unavailable: false, travel: { playBase: "/play/", jumpHref: "/play/?position=34%2C17" }, livePlayers, mobile: false };
  const router = createMemoryRouter(
    [{ id: "place", path: "/places/:id", Component: PlaceDetailRoute as never }],
    { initialEntries: [`/places/${PLACE_ID}`], hydrationData: { loaderData: { place: loaderData } } },
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
  return render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
}
const button = (name: string) => screen.getByRole("button", { name });
const pressed = () => ["Like", "Dislike", "Favorite"].map((name) => button(name).getAttribute("aria-pressed"));
const signIn = () => {
  auth.identity = IDENTITY;
  auth.isConnected = true;
};

beforeEach(() => {
  vi.clearAllMocks();
  auth.identity = null;
  auth.isConnected = false;
});
afterEach(cleanup);

it("signed out, the page shows the counts, reads nothing, and a press opens sign-in and goes back", async () => {
  const user = userEvent.setup();
  mount();
  expect(screen.getByRole("group", { name: "Rate and save this place" }).textContent).toBe("87% likes135 favorites");
  for (const name of ["Like", "Dislike", "Favorite"]) {
    await user.click(button(name));
    await waitFor(() => expect(pressed()).toEqual(["false", "false", "false"]));
  }
  expect(openSignIn).toHaveBeenCalledTimes(3);
  expect(signedGetJSON).not.toHaveBeenCalled();
  expect(postJSON).not.toHaveBeenCalled();
});

it("signed in, the visitor's own save is read with their identity and a like is their signed write", async () => {
  const user = userEvent.setup();
  signIn();
  mount(3);
  await waitFor(() => expect(pressed()).toEqual(["false", "false", "true"]));
  expect(signedGetJSON).toHaveBeenCalledWith(`/places/api/places/${PLACE_ID}`, expect.objectContaining({ identity: IDENTITY }));
  await user.click(button("Like"));
  expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: true }, { identity: IDENTITY, method: "PATCH" });
  await user.click(button("Favorite"));
  expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/favorites`, { favorites: false }, { identity: IDENTITY, method: "PATCH" });
  expect(pressed()).toEqual(["true", "false", "false"]);
  expect(openSignIn).not.toHaveBeenCalled();
});

it("a write the node refuses puts the button back", async () => {
  const user = userEvent.setup();
  signIn();
  mount();
  await waitFor(() => expect(pressed()).toEqual(["false", "false", "true"]));
  vi.mocked(postJSON).mockRejectedValueOnce(new Error("503"));
  await user.click(button("Dislike"));
  expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: false }, { identity: IDENTITY, method: "PATCH" });
  await waitFor(() => expect(pressed()).toEqual(["false", "false", "true"]));
  vi.mocked(postJSON).mockRejectedValueOnce(new Error("503"));
  await user.click(button("Favorite"));
  await waitFor(() => expect(pressed()).toEqual(["false", "false", "true"]));
  expect(postJSON).toHaveBeenCalledTimes(2);
});
