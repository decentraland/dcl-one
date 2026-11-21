import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import crew from "@dcl/one-ui/apps/sites/assets/errors/crew.webp";
import locked from "@dcl/one-ui/apps/sites/assets/errors/locked.webp";
import lost from "@dcl/one-ui/apps/sites/assets/errors/lost.webp";
import notFoundAvatar from "@dcl/one-ui/apps/sites/assets/errors/not-found-avatar.webp";
import wave from "@dcl/one-ui/apps/sites/assets/errors/wave.webp";
import { StatusPage } from "@dcl/one-ui/apps/sites/status/StatusPage";
import { ErrorBoundary } from "./root";

const routeError = (status: number, statusText: string, data: unknown = "") =>
  ({ status, statusText, internal: false, data }) as unknown;
// Every status the root renders with StatusPage (404 renders NotFoundPage), with the art it draws.
const ART_BY_STATUS: Record<number, string> = {
  400: wave, 401: locked, 403: locked, 405: wave, 408: wave, 410: lost, 413: wave, 414: wave, 418: wave, 429: wave,
  500: crew, 502: crew, 503: crew, 504: crew, 507: crew,
};
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
    expect(html).toContain(`src="${notFoundAvatar}"`);
    expect(html).toContain('aria-label="Decentraland Home"');
    expect(html).toMatch(/href="\/whats-on"[^>]*>(<span[^>]*>)*See Events</);
    expect(html).not.toContain("Nothing built on this parcel");
  });

  it("offers a retry on 5xx and uses the crew art", () => {
    const html = renderBoundary(routeError(503, "Service Unavailable"));
    expect(html).toContain("We&#x27;ll be right back");
    expect(html).toContain(`src="${crew}"`);
    expect(html).toContain("Try again");
  });

  it("offers Go back, not a retry, on a 4xx", () => {
    for (const status of [400, 401, 403, 410, 418]) {
      const html = renderBoundary(routeError(status, ""));
      expect(html, String(status)).toContain("Go back");
      expect(html, String(status)).not.toContain("Try again");
    }
  });

  it("offers a retry on the retryable client errors 408 and 429", () => {
    for (const status of [408, 429]) {
      const html = renderBoundary(routeError(status, ""));
      expect(html, String(status)).toContain("Try again");
      expect(html, String(status)).not.toContain("Go back");
    }
  });

  it("links the Decentraland logo and the primary action home", () => {
    const html = renderBoundary(routeError(503, "Service Unavailable"));
    expect(html).toMatch(/<a [^>]*href="\/"[^>]*><svg[^>]*>.*?<\/svg><span>Decentraland<\/span><\/a>/);
    expect(html).toMatch(/<a [^>]*href="\/"[^>]*>Go to homepage<\/a>/);
  });

  it("draws each status with its art", () => {
    for (const [status, art] of Object.entries(ART_BY_STATUS)) {
      expect(renderBoundary(routeError(Number(status), "")), status).toContain(`src="${art}"`);
    }
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
    expect(html).toContain(`src="${crew}"`);
    expect(html).not.toContain("boom");
  });
});

describe("when an English error replaces a localized route", () => {
  let html: string;

  beforeEach(() => {
    html = renderBoundary(routeError(503, "Service Unavailable"));
  });

  it("should identify the language of the error content", () => {
    expect(html).toMatch(/<div lang="en">[\s\S]*We&#x27;ll be right back/);
  });
});
