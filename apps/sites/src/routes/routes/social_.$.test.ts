import { describe, expect, it } from "vitest";

import { loader } from "./social_.$";

describe("/social/*", () => {
  it("answers 404 for any path under /social that is not a community page", () => {
    let thrown: unknown;
    try {
      loader();
    } catch (error) {
      thrown = error;
    }
    expect((thrown as { init?: { status?: number } }).init?.status).toBe(404);
  });
});
