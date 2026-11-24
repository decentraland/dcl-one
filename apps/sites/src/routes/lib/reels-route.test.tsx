// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ReelErrorBoundary } from "./reels-route";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

afterEach(cleanup);

describe("when a gallery address is invalid", () => {
  it("should show gallery-specific copy without a retry", async () => {
    const router = createMemoryRouter([
      {
        path: "/",
        loader() {
          throw new Response(null, { status: 404 });
        },
        errorElement: <ReelErrorBoundary missing="invalid-gallery" />,
        Component: () => null,
        HydrateFallback: () => null,
      },
    ]);
    render(<RouterProvider router={router} />);
    expect(await screen.findByText("Gallery not found")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });
});

describe("when Catalyst is temporarily unavailable", () => {
  it("should revalidate the route after the user retries", async () => {
    const loader = vi
      .fn()
      .mockRejectedValueOnce(new Response(null, { status: 503 }))
      .mockResolvedValueOnce(null);
    const router = createMemoryRouter([
      {
        path: "/",
        loader,
        errorElement: <ReelErrorBoundary />,
        Component: () => <p>Recovered photo</p>,
        HydrateFallback: () => null,
      },
    ]);
    render(<RouterProvider router={router} />);
    await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Recovered photo")).toBeTruthy();
    expect(loader).toHaveBeenCalledTimes(2);
  });
});
