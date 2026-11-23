import { describe, expect, it } from "vitest";

import {
  addressAvatarColor,
  avatarInitial,
  fnv1a32,
  getAvatarBackgroundColor,
  getDisplayName,
  syntheticAvatar,
} from "./avatarColor";

const svgOf = (dataUrl: string) => {
  expect(dataUrl.startsWith("data:image/svg+xml;utf8,")).toBe(true);
  return decodeURIComponent(dataUrl.slice("data:image/svg+xml;utf8,".length));
};

describe("fnv1a32", () => {
  it("hashes to the FNV-1a 32-bit values", () => {
    expect(fnv1a32("")).toBe(2166136261);
    expect(fnv1a32("Alice")).toBe(752715143);
  });
});

describe("getDisplayName", () => {
  it("keeps letters and digits and tags an unclaimed name with the address's last four characters", () => {
    expect(getDisplayName("Al ice!", false, "0x1234567890abcdef")).toBe("Alice#cdef");
  });

  it("leaves a claimed name, a missing address or a short address untagged", () => {
    expect(getDisplayName("Al ice!", true, "0xabcd1234")).toBe("Alice");
    expect(getDisplayName("Bob", false, null)).toBe("Bob");
    expect(getDisplayName("Bob", false, "0xab")).toBe("Bob");
  });

  it("is empty when nothing alphanumeric is left", () => {
    expect(getDisplayName("!!", false, "0xabcdef")).toBe("");
    expect(getDisplayName(null, false, null)).toBe("");
  });
});

describe("getAvatarBackgroundColor", () => {
  it("derives one colour per display name", () => {
    expect(getAvatarBackgroundColor("Alice")).toBe("#f5ff40");
  });

  it("falls back to white without a name", () => {
    expect(getAvatarBackgroundColor("")).toBe("#ffffff");
    expect(getAvatarBackgroundColor(null)).toBe("#ffffff");
  });
});

describe("syntheticAvatar", () => {
  it("draws the upper-cased first letter on the name's colour", () => {
    const svg = svgOf(syntheticAvatar("éclair"));
    expect(svg).toContain(`fill="${getAvatarBackgroundColor("éclair")}"`);
    expect(svg).toContain(">É</text>");
  });

  it("draws a question mark when the name has no letter or digit", () => {
    expect(svgOf(syntheticAvatar("!!"))).toContain(">?</text>");
  });
});

describe("addressAvatarColor and avatarInitial", () => {
  it("gives each address one hue at fixed saturation and lightness", () => {
    expect(addressAvatarColor("0xabc")).toBe("hsl(42, 52%, 45%)");
    expect(addressAvatarColor("")).toBe("hsl(0, 52%, 45%)");
  });

  it("uses the name's first letter, else the address's first hex digit, else a question mark", () => {
    expect(avatarInitial(" zed", "0xabc")).toBe("Z");
    expect(avatarInitial(null, "0xdef")).toBe("D");
    expect(avatarInitial("  ", "0X9")).toBe("9");
    expect(avatarInitial("", "")).toBe("?");
  });
});
