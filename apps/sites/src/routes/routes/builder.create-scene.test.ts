import { RouterContextProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { loader } from "./builder.create-scene";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

describe("/builder/create-scene", () => {
  describe("when a legacy Builder link asks to create a scene", () => {
    it("should redirect to the /create landing", async () => {
      const response = await loader({
        request: new Request("https://sites.test/builder/create-scene?ref=builder"),
        context: new RouterContextProvider(),
      } as never);

      expect(response.status).toBe(302);
      expect(response.headers.get("Location")).toBe("/create");
    });
  });
});
