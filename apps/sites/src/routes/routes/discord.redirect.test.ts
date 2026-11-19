import { describe, expect, it } from "vitest";

import { loader } from "./discord";

describe("/discord", () => {
  it("sends the visitor to the public Discord invite link", () => {
    const response = loader() as Response;
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("https://dcl.gg/discord");
  });
});
