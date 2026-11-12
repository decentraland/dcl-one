import { describe, expect, it } from "vitest";

import { DISCORD_URL, loader } from "./discord";

describe("/discord", () => {
  it("sends the visitor to the public Discord invite link", () => {
    const response = loader() as Response;
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(DISCORD_URL);
    expect(DISCORD_URL).toBe("https://dcl.gg/discord");
  });
});
