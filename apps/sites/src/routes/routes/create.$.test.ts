import { describe, expect, it } from "vitest";

import { loader } from "./create.$";

function load(splat: string) {
  return loader({ params: { "*": splat }, request: new Request(`https://sites.test/create/${splat}`) } as never);
}

describe("/create/* outside the pages Sites serves", () => {
  it.each(["home", "scenes", "scene-editor", "land", "names", "activity/example.dcl.eth"])(
    "/create/%s goes to the Creator Hub download page",
    (splat) => {
      const res = load(splat) as Response;
      expect(res.status).toBe(308);
      expect(res.headers.get("Location")).toBe("/download/creator-hub");
    },
  );

  it("answers 404 for anything else", () => {
    expect(() => load("nope")).toThrow();
    try {
      load("nope");
    } catch (thrown) {
      expect((thrown as Response).status).toBe(404);
    }
  });
});
