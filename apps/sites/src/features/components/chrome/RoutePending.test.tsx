/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

vi.hoisted(() =>
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true }),
);
const routerState = vi.hoisted(() => ({ navigation: "idle", revalidator: "idle" }));

vi.mock("react-router", async (importActual) => ({
  ...(await importActual<typeof import("react-router")>()),
  useNavigation: () => ({ state: routerState.navigation }),
  useRevalidator: () => ({ state: routerState.revalidator, revalidate: vi.fn() }),
}));

import RoutePending from "./RoutePending";

function renderWith(navigation: "idle" | "loading" | "submitting", revalidator: "idle" | "loading") {
  routerState.navigation = navigation;
  routerState.revalidator = revalidator;
  return render(<RoutePending />);
}

afterEach(() => {
  cleanup();
  routerState.navigation = "idle";
  routerState.revalidator = "idle";
});

describe("RoutePending", () => {
  it("renders nothing while navigation and revalidation are idle", () => {
    const { container } = renderWith("idle", "idle");
    expect(container.innerHTML).toBe("");
    expect(screen.queryByRole("status")).toBeNull();
  });

  it.each([
    ["loading", "idle", "Loading page…"],
    ["submitting", "idle", "Saving changes…"],
    ["idle", "loading", "Refreshing content…"],
    ["loading", "loading", "Loading page…"],
    ["submitting", "loading", "Saving changes…"],
  ] as const)("navigation %s, revalidator %s: announces %s in one polite status", (navigation, revalidator, label) => {
    renderWith(navigation, revalidator);
    const statuses = screen.getAllByRole("status");
    expect(statuses).toHaveLength(1);
    expect(statuses[0].getAttribute("aria-live")).toBe("polite");
    expect(statuses[0].textContent).toBe(label);
  });

  it("keeps the spinner and the progress bar out of the accessibility tree", () => {
    const { container } = renderWith("loading", "idle");
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByRole("status").querySelector('[aria-hidden="true"]')).not.toBeNull();
    const outside = [...container.children].filter((el) => el !== screen.getByRole("status"));
    expect(outside).toHaveLength(1);
    expect(outside[0].getAttribute("aria-hidden")).toBe("true");
  });
});
