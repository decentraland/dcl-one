import { describe, expect, it } from "vitest";

import { loader } from "./create.$";

describe("/create/* outside the pages Sites serves", () => {
  it("redirects to the /create landing", () => {
    const res = loader();
    expect(res.status).toBe(302);
    expect(res.headers.get("Location")).toBe("/create");
  });
});
