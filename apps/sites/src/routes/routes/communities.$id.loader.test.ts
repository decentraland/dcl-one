import { RouterContextProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { loadCommunity, type CommunityDetail } from "@/data/lib/catalyst/social/communities";
import { loader } from "./communities.$id";

vi.mock("@/data/lib/catalyst/social/communities", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/communities")>()),
  loadCommunity: vi.fn(),
}));

const args = (id: string, search = "") =>
  ({
    request: new Request(`https://sites.test/communities/${encodeURIComponent(id)}${search}`),
    params: { id },
    context: new RouterContextProvider(),
  }) as never;

describe("when a visitor opens /communities/:id", () => {
  afterEach(() => vi.resetAllMocks());

  it("should load the community named in the path", async () => {
    const detail = { community: { id: "c-1" } } as CommunityDetail;
    vi.mocked(loadCommunity).mockResolvedValue(detail);
    const { data } = await loader(args("c-1", "?tab=events"));
    expect(loadCommunity).toHaveBeenCalledWith("c-1", expect.anything());
    expect(data).toMatchObject({ id: "c-1", tab: "events", detail });
  });

  it("should show the not-found state when the community cannot be read", async () => {
    vi.mocked(loadCommunity).mockRejectedValue(new Error("down"));
    const { data } = await loader(args("c-1"));
    expect(data.detail).toBeNull();
  });

  it("should read nothing for a blank id", async () => {
    const { data } = await loader(args(" "));
    expect(loadCommunity).not.toHaveBeenCalled();
    expect(data).toMatchObject({ id: "", detail: null });
  });
});
