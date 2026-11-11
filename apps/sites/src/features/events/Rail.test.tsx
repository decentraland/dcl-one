// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

import { Rail } from "@dcl/one-ui/apps/sites/events/Rail";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/** A rail three viewports wide, so the next control shows. */
function renderRail(reduce: boolean) {
  vi.stubGlobal(
    "matchMedia",
    (query: string) => ({ matches: reduce && query.includes("reduce"), media: query }) as MediaQueryList,
  );
  const scrollTo = vi.fn();
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(300);
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(900);
  HTMLElement.prototype.scrollTo = scrollTo;
  render(
    <Rail label="live scenes" count={3}>
      <div>One</div>
    </Rail>,
  );
  return scrollTo;
}

describe("Rail", () => {
  it("should glide to the next page", async () => {
    const scrollTo = renderRail(false);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Show next live scenes" }));
    expect(scrollTo).toHaveBeenCalledWith({ left: 300, behavior: "smooth" });
  });

  it("should jump without animation for a visitor who asks for reduced motion", async () => {
    const scrollTo = renderRail(true);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Show next live scenes" }));
    expect(scrollTo).toHaveBeenCalledWith({ left: 300, behavior: "auto" });
  });
});
