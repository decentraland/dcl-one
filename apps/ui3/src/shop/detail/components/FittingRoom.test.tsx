// The fitting room draws the ticked items on a mannequin in the wearable-preview iframe, and a change of items reloads
// the scene in place, not the iframe.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { primeRemoteFlags } from "../../discovery/lib/remoteFlags";
import FittingRoom from "./FittingRoom";

const PREVIEW = "https://wearable-preview.decentraland.org";
const MALE = "urn:decentraland:off-chain:base-avatars:BaseMale";
const HOODIE = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:0";
const HAT = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1";

vi.mock("../lib/pickRenderer", () => ({ pickRenderer: () => ({ renderer: "full", reason: "connection-ok" }) }));
vi.mock("../../../wearable-preview/WearablePreview", () => ({
  default: () => {
    throw new Error("the fitting room must not draw the three.js scene");
  },
}));

const item = (id: string, urn: string, wearableCategory: string, name: string) =>
  ({ id, urn, name, category: "wearable", wearableCategory, thumbnail: `https://thumbs.example/${id}.png` }) as unknown as CatalogItem;
const ITEMS = [item("hoodie", HOODIE, "upper_body", "Hoodie"), item("hat", HAT, "hat", "Hat")];

const frame = () => screen.getByTitle("3D preview") as HTMLIFrameElement;
const params = () => new URL(frame().src).searchParams;

beforeEach(() => primeRemoteFlags({ "dapps-unity-wearable-preview": true }));
afterEach(() => {
  cleanup();
  primeRemoteFlags(null);
});

test("the ticked items go on a default mannequin through Unity's builder mode, and unticking one updates the scene in place", () => {
  render(<FittingRoom items={ITEMS} onClose={() => {}} portal={false} />);
  expect(params().getAll("urn")).toEqual([HOODIE, HAT]);
  expect(params().get("profile")).toBe("default");
  expect(params().get("bodyShape")).toBe(MALE);
  expect(params().get("mode")).toBe("builder");
  expect(screen.getByRole("status").textContent).toBeTruthy();

  const win = frame().contentWindow!;
  const posted = vi.spyOn(win, "postMessage");
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data: { type: "ready" }, source: win, origin: PREVIEW }));
  });
  fireEvent.click(screen.getByRole("checkbox", { name: /Hat/ }));
  expect(frame().contentWindow).toBe(win);
  expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "update", payload: { options: { urns: [HOODIE] } } });
});
