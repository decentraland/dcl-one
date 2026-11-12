import type { ComponentProps } from "react";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type PlaceDetail from "@ui/explorer/pages/PlaceDetail";

import type { PlaceTravel, WorldScene } from "@/data/lib/catalyst/places/world-scenes";
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

const PLACE_ID = "66ef7bd9-3060-4e1a-a6d7-edc8d251e0cb";

const GENESIS_PLACE = { id: PLACE_ID, title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false, world_name: null as string | null, contact_name: "Studio", owner: "0x296fa4213a6a88590328008dd5f5b535cf37bd66" };

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
  renderToString(<RouterProvider router={router} />);
  return seen.props!;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("/places/:id", () => {
  it("wires the visible travel control to the current parcel", () => {
    expect(renderRoute().jumpHref).toBe("/play/?position=34%2C17&realm=fixture");
  });

  it("hands PlaceDetail the place, its creator fields, the live player count and the device", () => {
    const props = renderRoute();
    expect(props.place).toMatchObject({ id: PLACE_ID, title: "New Scene 12", coords: "34,17", contactName: "Studio", owner: "0x296fa4213a6a88590328008dd5f5b535cf37bd66" });
    expect(props.livePlayers).toBe(3);
    expect(props.mobile).toBe(false);
  });
});

describe("when a world has more than one scene", () => {
  const world = { ...GENESIS_PLACE, id: "sample.dcl.eth", base_position: "0,0", positions: ["0,0"], world: true, world_name: "sample.dcl.eth" };
  const scenes = [{ entityId: "bafy1", title: "Plaza", base: "0,0" }, { entityId: "bafy2", title: "Arena", base: "10,4" }];
  let props: DetailProps;
  beforeEach(() => { props = renderRoute({ playBase: "/play/", jumpHref: "/play/?realm=sample.dcl.eth&position=0%2C0" }, world, scenes); });
  it("should offer each scene by title and base parcel, the first one picked", () => {
    expect(props.scenePicker).toMatchObject({ options: [{ value: "bafy1", label: "Plaza (0,0)" }, { value: "bafy2", label: "Arena (10,4)" }], value: "bafy1" });
    expect(props.jumpHref).toBe("/play/?realm=sample.dcl.eth&position=0%2C0");
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
