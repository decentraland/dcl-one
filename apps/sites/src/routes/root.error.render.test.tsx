import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";

import StatusPage from "@ui/components/StatusPage";
import { ErrorBoundary } from "./root";

const routeError = (status: number, statusText: string, data: unknown = "") =>
  ({ status, statusText, internal: false, data }) as unknown;
// The exported boundary reads its props from the data router, so seed one with the error.
const renderBoundary = (error: unknown) => {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", Component: () => null, ErrorBoundary: ErrorBoundary as never }],
    { hydrationData: { errors: { root: error } } },
  );
  return renderToString(<RouterProvider router={router} />);
};

describe("root error boundary", () => {
  it("renders decentraland.org's not-found page for a 404, with its art, home link and events link", () => {
    const html = renderBoundary(routeError(404, "Not Found", "No route matches /nope"));
    expect(html).toContain('data-status="404"');
    expect(html).toContain("This page has left the world.");
    expect(html).toContain("assets/errors/not-found-avatar.webp");
    expect(html).toContain('aria-label="Decentraland Home"');
    expect(html).toMatch(/href="\/whats-on"[^>]*>(<span[^>]*>)*See Events</);
    expect(html).not.toContain("Nothing built on this parcel");
  });

  it("offers a retry on 5xx and uses the crew art", () => {
    const html = renderBoundary(routeError(503, "Service Unavailable"));
    expect(html).toContain("We&#x27;ll be right back");
    expect(html).toContain("assets/errors/crew.webp");
    expect(html).toContain("Try again");
  });

  it("falls back to class copy for statuses without their own page", () => {
    expect(renderBoundary(routeError(418, "I'm a teapot"))).toContain("We couldn&#x27;t open that page");
    expect(renderBoundary(routeError(507, "Insufficient Storage"))).toContain("Something went wrong on our side");
  });

  it("shows response details only in development", () => {
    const dev = renderToString(<StatusPage status={404} detail="No route matches /nope" isDev />);
    const prod = renderToString(<StatusPage status={404} detail="No route matches /nope" />);
    expect(dev).toContain("Developer details");
    expect(dev).toContain("No route matches /nope");
    expect(prod).not.toContain("No route matches /nope");
  });

  it("renders crashes as the branded 500 screen in development too, the trace left to the error report", () => {
    const html = renderBoundary(new TypeError("boom"));
    expect(import.meta.env.DEV).toBe(true);
    expect(html).toContain('data-status="500"');
    expect(html).toContain("assets/errors/crew.webp");
    expect(html).not.toContain("boom");
  });
});
