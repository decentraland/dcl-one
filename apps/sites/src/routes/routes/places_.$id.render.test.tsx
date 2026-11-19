import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { PlaceDetailProps } from "@dcl/one-ui/apps/sites/places/PlaceDetail";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useSceneChat } from "@/features/explorer/scene-chat/useSceneChat";
import type { PlaceTravel, WorldScene } from "@/data/lib/catalyst/places/world-scenes";
import { track } from "@/core/lib/telemetry/track";
import PlaceDetailRoute from "./places_.$id";

const seen: { props: PlaceDetailProps | null } = { props: null };

vi.mock("@dcl/one-ui/apps/sites/places/PlaceDetail", () => ({
  PlaceDetail: (props: PlaceDetailProps) => {
    seen.props = props;
    return null;
  },
}));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/features/explorer/scene-chat/useSceneChat", () => ({ useSceneChat: vi.fn(() => ({ status: "loading", messages: [] })) }));
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

const GENESIS_PLACE = { id: PLACE_ID, title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false, world_name: null as string | null, contact_name: "Studio", owner: "0x296fa4213a6a88590328008dd5f5b535cf37bd66", like_rate: 0.874 as number | null, favorites: 135 };

function renderRoute(travel: PlaceTravel = { playBase: "/play/", jumpHref: "/play/?position=34%2C17&realm=fixture" }, place = GENESIS_PLACE, scenes: WorldScene[] = []) {
  seen.props = null;
  const loaderData = {
    place,
    scenes,
    sid: "sid-1",
    unavailable: false,
    travel,
    livePlayers: 3,
    mobile: false,
  };
  // The exported route reads its loader data from the data router, so seed one with it.
  const router = createMemoryRouter(
    [{ id: "place", path: "/places/:id", Component: PlaceDetailRoute as never }],
    { initialEntries: [`/places/${PLACE_ID}`], hydrationData: { loaderData: { place: loaderData } } },
  );
  const client = new QueryClient();
  renderToString(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
  return seen.props!;
}

beforeEach(() => {
  vi.clearAllMocks();
  auth.identity = null;
  auth.isConnected = false;
});

describe("/places/:id", () => {
  it("wires the visible travel control to the current parcel", () => {
    expect(renderRoute().jumpHref).toBe("/play/?position=34%2C17&realm=fixture");
  });

  it("hands PlaceDetail the place, its creator fields, the live player count and the device", () => {
    const props = renderRoute();
    expect(props.place).toMatchObject({ id: PLACE_ID, title: "New Scene 12", coords: "34,17" });
    expect(props.creator).toMatchObject({ name: "Studio", address: GENESIS_PLACE.owner });
    expect(props.livePlayers).toBe(3);
    expect(props.mobile).toBe(false);
  });
});

describe("/places/:id likes and favorites", () => {
  const signIn = () => {
    auth.identity = IDENTITY;
    auth.isConnected = true;
  };

  it("hands PlaceDetail the approval, the favorites count and the signed-in account", () => {
    expect(renderRoute().place).toMatchObject({ approval: 87, favorites: 135 });
    expect(renderRoute().account).toBeNull();
    expect(renderRoute(undefined, { ...GENESIS_PLACE, like_rate: null }).place).toMatchObject({ approval: null });
    signIn();
    expect(renderRoute().account).toBe(IDENTITY.signer);
  });

  it("saves a like, a dislike, a cleared rating and a favorite as the visitor's signed PATCHes to the node's places API", async () => {
    signIn();
    const props = renderRoute();
    await props.onLike!(PLACE_ID, true);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: true }, { identity: IDENTITY, method: "PATCH" });
    await props.onLike!(PLACE_ID, false);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: false }, { identity: IDENTITY, method: "PATCH" });
    await props.onLike!(PLACE_ID, null);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: null }, { identity: IDENTITY, method: "PATCH" });
    await props.onFavorite!(PLACE_ID, true);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/favorites`, { favorites: true }, { identity: IDENTITY, method: "PATCH" });
    expect(openSignIn).not.toHaveBeenCalled();
  });

  it("the server render knows no session, so PlaceDetail starts without the visitor's saved state and nothing is read", () => {
    expect(renderRoute().viewer).toBeNull();
    signIn();
    expect(renderRoute().viewer).toBeNull();
    expect(signedGetJSON).not.toHaveBeenCalled();
  });

  it("signed out, a like, a dislike or a favorite opens sign-in and rejects so the button reverts", async () => {
    const props = renderRoute();
    await expect(props.onLike!(PLACE_ID, true)).rejects.toThrow(/Sign in/);
    await expect(props.onLike!(PLACE_ID, false)).rejects.toThrow(/Sign in/);
    await expect(props.onFavorite!(PLACE_ID, true)).rejects.toThrow(/Sign in/);
    expect(openSignIn).toHaveBeenCalledTimes(3);
    expect(postJSON).not.toHaveBeenCalled();
  });

  it("an expired session is signed out: the write is not sent with its identity", async () => {
    auth.identity = IDENTITY;
    const props = renderRoute();
    expect(props.account).toBeNull();
    await expect(props.onFavorite!(PLACE_ID, true)).rejects.toThrow(/Sign in/);
    expect(postJSON).not.toHaveBeenCalled();
  });

  it("a write the node refuses rejects, so the button reverts, and sign-in stays closed", async () => {
    signIn();
    vi.mocked(postJSON).mockRejectedValueOnce(new Error("403"));
    await expect(renderRoute().onLike!(PLACE_ID, true)).rejects.toThrow("403");
    expect(openSignIn).not.toHaveBeenCalled();
  });

  it("the scene chat stays a guest: it gets the room's target and nothing of the visitor's session", () => {
    signIn();
    renderRoute();
    expect(vi.mocked(useSceneChat).mock.calls).toEqual([[{ location: "34,17" }]]);
    expect(seen.props!.chat).toEqual({ status: "loading", messages: [] });
  });
});

describe("when a world has more than one scene", () => {
  const world = { ...GENESIS_PLACE, id: "sample.dcl.eth", base_position: "0,0", positions: ["0,0"], world: true, world_name: "sample.dcl.eth" };
  const scenes = [{ entityId: "bafy1", title: "Plaza", base: "0,0" }, { entityId: "bafy2", title: "Arena", base: "10,4" }];
  let props: PlaceDetailProps;
  beforeEach(() => { props = renderRoute({ playBase: "/play/", jumpHref: "/play/?realm=sample.dcl.eth&position=0%2C0" }, world, scenes); });
  it("should offer each scene by title and base parcel, the first one picked", () => {
    expect(props.scenePicker).toMatchObject({ options: [{ value: "bafy1", label: "Plaza (0,0)" }, { value: "bafy2", label: "Arena (10,4)" }], value: "bafy1" });
    expect(props.jumpHref).toBe("/play/?realm=sample.dcl.eth&position=0%2C0");
  });
  it("should not join the world's room: the chat gets no target and the pane is unavailable", () => {
    expect(vi.mocked(useSceneChat).mock.calls).toEqual([[null]]);
    expect(props.chat).toEqual({ status: "unavailable", messages: [] });
  });
});

describe("when handing off place travel", () => {
  let props: PlaceDetailProps;
  beforeEach(() => { props = renderRoute(); });
  afterEach(() => vi.clearAllMocks());
  it("should retain started and completed launch telemetry", () => {
    props.onJumpIn!();
    expect(track).toHaveBeenCalledWith("jump_in_started", { place_id: PLACE_ID }, expect.anything());
    expect(track).toHaveBeenCalledWith("jump_in_completed", { place_id: PLACE_ID, launch_url: props.jumpHref }, expect.anything());
  });
  describe("and the realm lookup failed", () => {
    beforeEach(() => { props = renderRoute({ playBase: "/play/", launchError: "Travel is temporarily unavailable. Please retry." }); });
    it("should retain failed telemetry and offer resolution again without a wrong-realm link", () => {
      props.onJumpIn!();
      expect(props.jumpHref).toBeUndefined();
      expect(track).toHaveBeenCalledWith("jump_in_failed", { place_id: PLACE_ID, error: "Travel is temporarily unavailable. Please retry." }, expect.anything());
      expect(track).not.toHaveBeenCalledWith("jump_in_completed", expect.anything(), expect.anything());
    });
  });
});
