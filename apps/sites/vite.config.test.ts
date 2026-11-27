import { describe, expect, it } from "vitest";
import config from "./vite.config";
import { LEGAL_SIDEBAR } from "./src/data/legal/nav";
const matched = (pathname: string) =>
  Object.keys(config.server?.proxy ?? {}).filter((prefix) =>
    prefix.startsWith("^")
      ? new RegExp(prefix).test(pathname)
      : pathname.startsWith(prefix),
  );
describe("when legal pages share a prefix with the node credits API", () => {
  it.each(LEGAL_SIDEBAR)("should keep /$slug on the Sites router", ({ slug }) => {
    expect(matched(`/${slug}`)).toEqual([]);
    expect(matched(`/${slug}?language=en`)).toEqual([]);
  });
  it.each(["/credits", "/credits?limit=1", "/credits/account"])(
    "should preserve the node API proxy at %s",
    (pathname) => {
      expect(matched(pathname)).toContain("^/credits(?:[/?]|$)");
    },
  );
});
describe("when the web Explorer signs in to a world's room", () => {
  it.each(["/worlds/name.dcl.eth/comms", "/worlds/name.dcl.eth/scenes/bafkreiscene/comms"])(
    "should send %s to the node",
    (pathname) => {
      expect(matched(pathname)).toEqual(["^/worlds/[^/]+/(?:scenes/[^/]+/)?comms(?:[/?]|$)"]);
    },
  );
  it.each(["/worlds", "/worlds/name.dcl.eth", "/worlds/name.dcl.eth/commsx"])(
    "should keep %s off the node proxy",
    (pathname) => {
      expect(matched(pathname)).toEqual([]);
    },
  );
});
