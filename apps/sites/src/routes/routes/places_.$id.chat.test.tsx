// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { postJSON } from "@/data/lib/catalyst/client";
import PlaceDetailRoute from "./places_.$id";
import WorldDetailRoute from "./places_.world.$name";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const signedFetch = vi.fn();
const rooms: Array<{ connect: ReturnType<typeof vi.fn> }> = [];
vi.mock("livekit-client", () => ({
  RoomEvent: { DataReceived: "dataReceived" },
  Room: class {
    connect = vi.fn(async () => {});
    disconnect = vi.fn(async () => {});
    constructor() {
      rooms.push(this);
    }
    on() {}
  },
}));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/data/lib/auth/signer", () => ({ signedFetch: (...args: unknown[]) => signedFetch(...args) }));
vi.mock("@/data/lib/catalyst/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/catalyst/client")>()),
  catalystBase: () => "https://node.test",
  getJSON: async (path: string, opts: { signal?: AbortSignal }) => (await fetch(`https://node.test${path}`, opts)).json(),
  postJSON: vi.fn(async () => ({ ok: true })),
  signedGetJSON: vi.fn(async () => ({ ok: true, data: { user_like: false, user_dislike: false, user_favorite: false } })),
}));
const IDENTITY: AuthIdentity = {
  signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295",
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: IDENTITY, isConnected: true }) }));

const GENESIS = { id: "66ef7bd9-3060-4e1a-a6d7-edc8d251e0cb", title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false, world_name: null as string | null, contact_name: "Studio", owner: null, like_rate: 0.874, favorites: 135 };
const WORLD = { ...GENESIS, id: "0b1e7a52-5a0c-4d8e-9a53-2f6d1c7e4a10", title: "Cozy", base_position: "0,0", positions: ["0,0"], world: true, world_name: "cozy.dcl.eth" };
const json = (body: unknown) => new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });

/** The signed-in desktop visitor's page of a place with three players in it, on a node that has a room for every scene. */
function mount(place: typeof GENESIS) {
  const scenes = place.world ? [
    { entityId: "bafyworld", title: "Plaza", base: "0,0" },
    { entityId: "bafyarena", title: "Arena", base: "10,4" },
  ] : [];
  const loaderData = { place, scenes, sid: "sid-1", unavailable: false, travel: { playBase: "/play/", jumpHref: "/play/" }, livePlayers: 3, mobile: false };
  const router = createMemoryRouter(
    [
      { id: "place", path: "/places/:id", Component: PlaceDetailRoute as never },
      { id: "world", path: "/places/world/:name", Component: WorldDetailRoute as never },
    ],
    {
      initialEntries: [place.world ? `/places/world/${place.world_name}` : `/places/${place.id}`],
      hydrationData: { loaderData: { [place.world ? "world" : "place"]: loaderData } },
    },
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
  return render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
}
const requested: string[] = [];

beforeEach(() => {
  rooms.length = 0;
  requested.length = 0;
  signedFetch.mockReset().mockImplementation(async () => json({ adapter: "livekit:wss://lk.test?access_token=jwt" }));
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = String(input);
    requested.push(url);
    if (url.endsWith("/content/entities/active")) return json([{ id: "bafyscene" }]);
    if (url.endsWith("/about")) return json({ configurations: { realmName: "node-realm" } });
    return new Response("{}", { status: 404 });
  });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.mocked(postJSON).mockClear();
});

it("world place: no join, the pane is unavailable and a like stays the visitor's signed write", async () => {
  const user = userEvent.setup();
  mount(WORLD);
  expect(screen.getByRole("heading", { name: "In-World Chat" })).toBeTruthy();
  expect(screen.getByText("Chat is unavailable right now.")).toBeTruthy();
  await user.selectOptions(screen.getByRole("combobox", { name: "Scene" }), "bafyarena");
  expect(screen.getByRole("link", { name: "Jump in" }).getAttribute("href")).toBe("/play/?realm=cozy.dcl.eth&position=10%2C4");
  await user.click(screen.getByRole("button", { name: "Like" }));
  expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${WORLD.id}/likes`, { like: true }, { identity: IDENTITY, method: "PATCH" });
  expect(signedFetch).not.toHaveBeenCalled();
  expect(requested.filter((url) => /comms-gatekeeper|entities\/active|\/about$/.test(url))).toEqual([]);
  expect(rooms).toHaveLength(0);
});

it("Genesis place: join as guest, never as the visitor's wallet", async () => {
  mount(GENESIS);
  await waitFor(() => expect(rooms).toHaveLength(1));
  expect(rooms[0].connect).toHaveBeenCalledWith("wss://lk.test/", "jwt", { autoSubscribe: false });
  expect(signedFetch).toHaveBeenCalledTimes(1);
  const [guest, url, init] = signedFetch.mock.calls[0] as [AuthIdentity, string, { metadata: unknown }];
  expect(url).toBe("https://node.test/comms-gatekeeper/get-scene-adapter");
  expect(init.metadata).toEqual({ sceneId: "bafyscene", parcel: "34,17", realmName: "node-realm", signer: "dcl:explorer" });
  expect(guest.signer).toMatch(/^0x[0-9a-f]{40}$/);
  expect(guest.signer).not.toBe(IDENTITY.signer);
  expect(requested).toEqual(["https://node.test/content/entities/active", "https://node.test/about"]);
  expect(screen.getByText("No messages yet.")).toBeTruthy();
});
