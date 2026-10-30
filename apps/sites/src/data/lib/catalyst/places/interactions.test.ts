import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../../auth/types";
import { signedGetJSON } from "../client";
import { loadPlaceViewer } from "./interactions";

vi.mock("../client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../client")>()),
  signedGetJSON: vi.fn(),
}));

const IDENTITY: AuthIdentity = {
  signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295",
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
const PLACE_ID = "3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d";
const answer = (row: { user_like: boolean; user_dislike: boolean; user_favorite: boolean }) =>
  vi.mocked(signedGetJSON).mockResolvedValueOnce({ ok: true, data: { id: PLACE_ID, favorites: 135, ...row } });

beforeEach(() => vi.mocked(signedGetJSON).mockReset());

describe("loadPlaceViewer", () => {
  it("reads the account's own like and save from the node's place, signed as the visitor", async () => {
    const signal = new AbortController().signal;
    answer({ user_like: false, user_dislike: false, user_favorite: true });
    await expect(loadPlaceViewer(IDENTITY, PLACE_ID, signal)).resolves.toEqual({ liked: null, favorite: true });
    expect(signedGetJSON).toHaveBeenCalledWith(`/places/api/places/${PLACE_ID}`, { identity: IDENTITY, signal });
  });

  it("a like reads as liked, a dislike as disliked", async () => {
    answer({ user_like: true, user_dislike: false, user_favorite: false });
    await expect(loadPlaceViewer(IDENTITY, PLACE_ID)).resolves.toEqual({ liked: true, favorite: false });
    answer({ user_like: false, user_dislike: true, user_favorite: false });
    await expect(loadPlaceViewer(IDENTITY, PLACE_ID)).resolves.toEqual({ liked: false, favorite: false });
  });

  it("an answer without the viewer fields is refused rather than read as not saved", async () => {
    vi.mocked(signedGetJSON).mockResolvedValueOnce({ ok: true, data: { id: PLACE_ID } });
    await expect(loadPlaceViewer(IDENTITY, PLACE_ID)).rejects.toThrow();
  });
});
