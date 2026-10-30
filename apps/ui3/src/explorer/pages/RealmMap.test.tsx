import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { FakeBridge } from "../../test/fakeBridge";
import RealmMap from "./RealmMap";

const ENTITIES = [
  { id: "a", pointers: ["0,0", "1,0"], metadata: { display: { title: "Plaza" }, scene: { base: "0,0" }, spawnPoints: [{ name: "gate", default: true, position: { x: 8, y: 0, z: 8 } }] } },
  { id: "b", pointers: ["3,2"], metadata: { display: { title: "Tower" }, scene: { base: "3,2" } } },
];

function serve() {
  vi.stubGlobal("fetch", vi.fn(async (url: string | URL | Request) => {
    const u = String(url);
    if (u.endsWith("/about")) return new Response(JSON.stringify({ configurations: { localSceneParcels: ["0,0", "1,0", "3,2"] } }));
    if (u.endsWith("/entities/active")) return new Response(JSON.stringify(ENTITIES));
    return new Response("nope", { status: 404 });
  }));
}

afterEach(() => { vi.unstubAllGlobals(); delete window.dclBridge; });

test("draws every scene of the realm, the player, and jumps to a clicked parcel", async () => {
  serve();
  const bridge = new FakeBridge();
  window.dclBridge = bridge;
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const { container } = render(<QueryClientProvider client={client}><RealmMap base="http://localhost:8002" coords="1,0" heading={90} /></QueryClientProvider>);
  const legend = within(await screen.findByRole("complementary", { name: "Scenes in this realm" }));
  expect(await legend.findByText("Plaza")).toBeInTheDocument();
  expect(legend.getByText("Tower")).toBeInTheDocument();
  expect(container.querySelectorAll(".rm__cell")).toHaveLength(3);
  expect(container.querySelector(".rm__player")).toHaveAttribute("aria-label", "You at 1,0");
  const tower = [...container.querySelectorAll(".rm__cell")].find((c) => c.querySelector("title")?.textContent?.includes("Tower"))!;
  fireEvent.click(tower);
  const card = within(screen.getByRole("dialog", { name: "Parcel 3,2" }));
  fireEvent.click(card.getByRole("button", { name: "Jump here" }));
  expect(bridge.sent.at(-1)).toEqual({ action: "Teleport", payload: { x: 56, z: 40 } });
  fireEvent.click(legend.getByText("Plaza"));
  fireEvent.click(within(screen.getByRole("dialog", { name: "Parcel 0,0" })).getByRole("button", { name: "gate" }));
  expect(bridge.sent.at(-1)).toEqual({ action: "Teleport", payload: { x: 8, z: 8 } });
});

test("the mini variant is a plain square around the player with no legend or labels", async () => {
  serve();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const { container } = render(<QueryClientProvider client={client}><RealmMap base="http://localhost:8002" coords="0,0" variant="mini" /></QueryClientProvider>);
  await screen.findByRole("img", { name: /Realm map/ });
  expect(screen.queryByRole("complementary")).toBeNull();
  expect(container.querySelector(".rm__label")).toBeNull();
  expect(container.querySelector(".rm__svg")).toHaveAttribute("viewBox", "0 0 13 13");
});
