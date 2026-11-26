import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({ openSignIn: vi.fn() }));
vi.mock("../../components/auth/signin-store", () => ({ openSignIn: state.openSignIn }));

import { emptyDraft } from "@/data/lib/catalyst/social/create-community";

import { submitWith, takeDraft } from "./submit";

const input = { name: "Builders", description: "We build", privacy: "private" as const, visibility: "unlisted" as const };
const trackCtx = { sid: "sid-1", story: "landings/create-community" as const, variant: "wizard" };
const RESULT = { id: "c1", name: "Builders", privacy: "private" as const, visibility: "unlisted" as const };
const store = new Map<string, string>();

beforeEach(() => {
  store.clear();
  vi.stubGlobal("sessionStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key),
  });
});
afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("submitWith", () => {
  describe("when the visitor is signed out", () => {
    it("should keep the form, open the sign-in and reject without sending", async () => {
      const track = vi.fn();
      await expect(submitWith(undefined, { signedOut: true, trackCtx, track })(input)).rejects.toThrow("Sign in to create a community.");
      expect(state.openSignIn).toHaveBeenCalledTimes(1);
      expect(takeDraft()).toEqual(input);
      expect(track).not.toHaveBeenCalled();
    });

    it("should keep the form as typed, not the description the submit fills in", async () => {
      const sent = { ...input, description: "Builders" };
      const typed = { ...input, description: "" };
      await expect(submitWith(undefined, { signedOut: true, trackCtx })(sent, typed)).rejects.toThrow("Sign in");
      expect(takeDraft()).toEqual(typed);
    });

    it("should still open the sign-in when the tab refuses storage", async () => {
      vi.stubGlobal("sessionStorage", {
        setItem: () => {
          throw new Error("quota");
        },
      });
      await expect(submitWith(undefined, { signedOut: true, trackCtx })(input)).rejects.toThrow("Sign in");
      expect(state.openSignIn).toHaveBeenCalledTimes(1);
    });
  });

  describe("while the session is still being restored", () => {
    it("should reject without sending or leaving for the sign-in", async () => {
      await expect(submitWith(undefined, { signedOut: false, trackCtx })(input)).rejects.toThrow("Still checking your sign-in");
      expect(state.openSignIn).not.toHaveBeenCalled();
      expect(takeDraft()).toBeUndefined();
    });
  });

  describe("when the visitor is signed in", () => {
    it("should send the form as a draft, report the attempt and the new community, and return it", async () => {
      const create = vi.fn().mockResolvedValue(RESULT);
      const track = vi.fn();
      await expect(submitWith(create, { signedOut: false, trackCtx, track })(input)).resolves.toBe(RESULT);
      expect(create).toHaveBeenCalledWith({ draft: { ...emptyDraft(), ...input } });
      expect(track.mock.calls).toEqual([
        ["lp_community_submit_attempted", { privacy: "private", visibility: "unlisted" }, trackCtx],
        ["lp_community_created", { community_id: "c1" }, trackCtx],
      ]);
      expect(state.openSignIn).not.toHaveBeenCalled();
    });

    it("should report and pass on the create's failure", async () => {
      const failure = Object.assign(new Error("nope"), { status: 500 });
      const track = vi.fn();
      await expect(submitWith(vi.fn().mockRejectedValue(failure), { signedOut: false, trackCtx, track })(input)).rejects.toBe(failure);
      expect(track).toHaveBeenLastCalledWith("lp_community_submit_failed", { error: "nope" }, trackCtx);
    });
  });
});

describe("takeDraft", () => {
  it("should return nothing when no draft was kept", () => {
    expect(takeDraft()).toBeUndefined();
  });

  it("should hand the draft back once", () => {
    store.set("dcl.create-community.draft", JSON.stringify(input));
    expect(takeDraft()).toEqual(input);
    expect(takeDraft()).toBeUndefined();
  });

  it("should drop fields of the wrong shape", () => {
    store.set("dcl.create-community.draft", JSON.stringify({ name: "Builders", description: 4, privacy: "secret", visibility: "all" }));
    expect(takeDraft()).toEqual({ name: "Builders", visibility: "all" });
  });

  it("should return nothing for an unreadable draft", () => {
    store.set("dcl.create-community.draft", "{not json");
    expect(takeDraft()).toBeUndefined();
    store.set("dcl.create-community.draft", "null");
    expect(takeDraft()).toBeUndefined();
  });

  it("should return nothing when storage is unavailable", () => {
    vi.stubGlobal("sessionStorage", {
      getItem: () => {
        throw new Error("denied");
      },
    });
    expect(takeDraft()).toBeUndefined();
  });
});
