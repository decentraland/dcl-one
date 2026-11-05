// @vitest-environment node
import { renderToString } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

vi.mock("./profileCache", () => ({
  cachedProfile: () => ({ name: "Doki3D", picture: null, claimed: true }),
  rememberProfiles: () => {},
}));

const { useCreatorProfile } = await import("../hooks");

function Name({ address }: { address: string }) {
  const { profile, loading } = useCreatorProfile(address);
  return <span>{loading ? "loading" : (profile?.name ?? "none")}</span>;
}

describe("the profile cache on the server", () => {
  it("is not read while rendering on the server, so the markup matches the first client render", () => {
    expect(renderToString(<Name address="0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295" />)).toContain("loading");
  });
});
