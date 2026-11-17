// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createRoutesStub } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PlacesTabs, type PlacesTab } from "./PlacesTabs";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

function renderTabs(active: PlacesTab) {
  const Stub = createRoutesStub([{ path: "/", Component: () => <PlacesTabs active={active} /> }]);
  render(<Stub initialEntries={["/"]} />);
}

afterEach(cleanup);

describe("PlacesTabs", () => {
  it("should link each tab to its route", () => {
    renderTabs("places");
    expect(screen.getByRole("tab", { name: "Places" }).getAttribute("href")).toBe("/places");
    expect(screen.getByRole("tab", { name: "Communities" }).getAttribute("href")).toBe("/places/communities");
  });

  it("should select only the active tab and keep it alone in the tab order", () => {
    renderTabs("places");
    const places = screen.getByRole("tab", { name: "Places" });
    const communities = screen.getByRole("tab", { name: "Communities" });
    expect(places.getAttribute("aria-selected")).toBe("true");
    expect(places.getAttribute("tabindex")).toBe("0");
    expect(communities.getAttribute("aria-selected")).toBe("false");
    expect(communities.getAttribute("tabindex")).toBe("-1");
  });
});
