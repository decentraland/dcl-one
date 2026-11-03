import type { ComponentProps } from "react";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type PlaceDetail from "@ui/explorer/pages/PlaceDetail";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { openSignIn } from "@/features/components/auth/signin-store";
import type { PlaceTravel } from "@/data/lib/catalyst/places/world-scenes";
import { track } from "@/core/lib/telemetry/track";
import PlaceDetailRoute from "./places_.$id";

type DetailProps = NonNullable<ComponentProps<typeof PlaceDetail>>;
const seen: { props: DetailProps | null } = { props: null };

vi.mock("@ui/explorer/pages/PlaceDetail", () => ({
  default: (props: DetailProps) => {
    seen.props = props;
    return null;
  },
}));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
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

function renderRoute(travel: PlaceTravel = { playBase: "/play/", jumpHref: "/play/?position=34%2C17&realm=fixture" }) {
  seen.props = null;
  const loaderData = {
    id: PLACE_ID,
    place: { id: PLACE_ID, title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false },
    sid: "sid-1",
    unavailable: false,
    travel,
  };
  // The exported route reads its loader data from the data router, so seed one with it.
  const router = createMemoryRouter(
    [{ id: "place", path: "/places/:id", Component: PlaceDetailRoute as never }],
    { initialEntries: [`/places/${PLACE_ID}`], hydrationData: { loaderData: { place: loaderData } } },
  );
  renderToString(<RouterProvider router={router} />);
  return seen.props!;
}

beforeEach(() => {
  vi.clearAllMocks();
  auth.identity = null;
  auth.isConnected = false;
});

describe("/places/:id likes and favorites", () => {
  it("wires the visible travel control to the current parcel", () => {
    expect(renderRoute().jumpHref).toBe("/play/?position=34%2C17&realm=fixture");
  });

  it("hands PlaceDetail the place id and saves signed PATCHes to the node's places API", async () => {
    auth.identity = IDENTITY;
    auth.isConnected = true;
    const props = renderRoute();
    expect(props.place?.id).toBe(PLACE_ID);

    await props.onFavorite!(PLACE_ID, true);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/favorites`, { favorites: true }, { identity: IDENTITY, method: "PATCH" });
    await props.onLike!(PLACE_ID, false);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: false }, { identity: IDENTITY, method: "PATCH" });
    await props.onLike!(PLACE_ID, null);
    expect(postJSON).toHaveBeenLastCalledWith(`/places/api/places/${PLACE_ID}/likes`, { like: null }, { identity: IDENTITY, method: "PATCH" });
  });

  it("the server render knows no session, so PlaceDetail starts without the visitor's saved state and nothing is read", () => {
    expect(renderRoute().viewer).toBeNull();
    auth.identity = IDENTITY;
    auth.isConnected = true;
    expect(renderRoute().viewer).toBeNull();
    expect(signedGetJSON).not.toHaveBeenCalled();
  });

  it("signed out, a like or favorite opens sign-in and rejects so the button reverts", async () => {
    const props = renderRoute();
    await expect(props.onFavorite!(PLACE_ID, true)).rejects.toThrow(/Sign in/);
    await expect(props.onLike!(PLACE_ID, true)).rejects.toThrow(/Sign in/);
    expect(openSignIn).toHaveBeenCalledTimes(2);
    expect(postJSON).not.toHaveBeenCalled();
  });
});


describe("when handing off place travel", () => {
  let props: DetailProps;
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
