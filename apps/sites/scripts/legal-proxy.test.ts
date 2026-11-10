import { describe, expect, it } from "vitest";
import config from "../vite.config";
import { LEGAL_DOCS } from "../src/data/legal";
const matched = (pathname: string) =>
  Object.keys(config.server?.proxy ?? {}).filter((prefix) =>
    prefix.startsWith("^")
      ? new RegExp(prefix).test(pathname)
      : pathname.startsWith(prefix),
  );
describe("when legal pages share a prefix with the node credits API", () => {
  it.each(Object.values(LEGAL_DOCS))(
    "should keep /$slug on the Sites router",
    (doc) => {
      expect(matched(`/${doc.slug}`)).toEqual([]);
      expect(matched(`/${doc.slug}?language=en`)).toEqual([]);
    },
  );
  it.each(["/credits", "/credits?limit=1", "/credits/account"])(
    "should preserve the node API proxy at %s",
    (pathname) => {
      expect(matched(pathname)).toContain("^/credits(?:[/?]|$)");
    },
  );
});
