import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError } from "../client";
import { createCommunity } from "./community-commit";
import { emptyDraft } from "./create-community";

vi.mock("../../auth/signer", () => ({
  signedFetch: (_identity: unknown, url: string, init: RequestInit) => fetch(url, init),
}));

const identity = {} as AuthIdentity;
const draft = { ...emptyDraft(), name: " Builders ", description: " We build ", privacy: "private" as const, visibility: "unlisted" as const };
let respond: () => Promise<Response>;

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(() => respond()));
});
afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("createCommunity", () => {
  describe("when the server creates the community", () => {
    beforeEach(() => {
      respond = async () => new Response(JSON.stringify({ data: { id: "c1", name: "Builders" } }), { status: 201 });
    });

    it("should post the trimmed draft and return the new id and name", async () => {
      await expect(createCommunity(draft, null, { identity, base: "https://node.test" })).resolves.toEqual({ id: "c1", name: "Builders" });
      const [url, init] = vi.mocked(fetch).mock.calls[0]!;
      expect(url).toBe("https://node.test/v1/communities");
      const body = (init as RequestInit).body as FormData;
      expect([body.get("name"), body.get("description"), body.get("privacy"), body.get("visibility")]).toEqual(["Builders", "We build", "private", "unlisted"]);
    });
  });

  describe("when the server refuses the signer (401)", () => {
    beforeEach(() => {
      respond = async () => new Response(JSON.stringify({ message: "The user doesn't have any names" }), { status: 401 });
    });

    it("should reject with the status and the server's message", async () => {
      const created = createCommunity(draft, null, { identity, base: "https://node.test" });
      await expect(created).rejects.toBeInstanceOf(CatalystError);
      await expect(created).rejects.toMatchObject({ status: 401, message: "Create community failed (401): The user doesn't have any names" });
    });
  });

  describe("when the request cannot reach the server", () => {
    beforeEach(() => {
      respond = () => Promise.reject(new Error("offline"));
    });

    it("should reject with no status", async () => {
      await expect(createCommunity(draft, null, { identity, base: "https://node.test" })).rejects.toMatchObject({
        status: 0,
        message: "Create community failed: offline",
      });
    });
  });

  describe("when the response names no community", () => {
    beforeEach(() => {
      respond = async () => new Response(JSON.stringify({ data: {} }), { status: 200 });
    });

    it("should reject", async () => {
      await expect(createCommunity(draft, null, { identity, base: "https://node.test" })).rejects.toThrow("response had no community id");
    });
  });
});
