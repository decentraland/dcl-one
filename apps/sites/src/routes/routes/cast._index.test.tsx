import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import CastIndexRoute from "./cast._index";

describe("when the /cast route renders", () => {
  let html: string;
  beforeEach(() => {
    const router = createMemoryRouter([{ id: "cast", path: "/cast", Component: CastIndexRoute as never }], {
      initialEntries: ["/cast"],
      hydrationData: { loaderData: { cast: { sid: "test", from: "unknown", reason: "missing", assignment: { variant: "instrumented" } } } },
    });
    html = renderToString(<RouterProvider router={router} />);
  });
  it("should display the Cast not-found heading", () => {
    expect(html).toContain("404 - Page Not Found");
  });
  it("should link to the Cast documentation", () => {
    expect(html).toContain("https://docs.decentraland.org/creator/worlds/cast/");
  });
});
