// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { emptyDraft, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import { openSignIn } from "../../components/auth/signin-store";
import { saveHangoutSignInDraft, takeHangoutSignInDraft, useHangoutSignInDraft } from "./sign-in-draft";

const EDITOR = "/events/new-event";
const KEY = "dcl:hangout:sign-in-draft:v2";
const OWNER = "0x1111111111111111111111111111111111111111";
const OTHER = "0x2222222222222222222222222222222222222222";

describe("when the hangout editor keeps its draft across a browser sign-in", () => {
  let draft: HangoutDraft;

  beforeEach(() => {
    draft = { ...emptyDraft(), name: "Moon party", contact: "host@example.test", imagePreviewUrl: "https://example.test/cover.png" };
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    sessionStorage.clear();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe("and the visitor comes back to the same editor", () => {
    beforeEach(() => { saveHangoutSignInDraft(EDITOR, { account: OWNER, draft }); });

    it("should give the draft and its account back once", () => {
      expect(takeHangoutSignInDraft(EDITOR)).toEqual({ account: OWNER, draft });
      expect(takeHangoutSignInDraft(EDITOR)).toBeNull();
    });
  });

  it("should keep every field of a fully filled draft", () => {
    const full: HangoutDraft = {
      name: "n", description: "d", startDate: "2099-01-01", startTime: "10:00", durationHours: 3, allDay: true, location: "world",
      coordX: 4, coordY: -5, worldName: "w.dcl.eth", recurrent: true, recurrence: "every_month", recurrentUntil: "2099-02-01",
      recurrentCount: 4, category: "music", communityId: "c", contact: "c@example.test", details: "x",
      imagePreviewUrl: "https://example.test/a.png", verticalImagePreviewUrl: "https://example.test/b.png", featuredItem: "urn:x",
    };
    saveHangoutSignInDraft(EDITOR, { account: OWNER, draft: full });
    expect(takeHangoutSignInDraft(EDITOR)?.draft).toEqual(full);
  });

  describe("and the visitor lands on another editor", () => {
    beforeEach(() => { saveHangoutSignInDraft(EDITOR, { account: OWNER, draft }); });

    it("should give nothing and keep the draft for its own editor", () => {
      expect(takeHangoutSignInDraft("/events/edit-event/other")).toBeNull();
      expect(takeHangoutSignInDraft(EDITOR)?.draft).toEqual(draft);
    });
  });

  describe("and the draft is older than the sign-in window", () => {
    beforeEach(() => {
      vi.useFakeTimers({ toFake: ["Date"] });
      saveHangoutSignInDraft(EDITOR, { account: OWNER, draft });
      vi.setSystemTime(Date.now() + 31 * 60 * 1000);
    });

    it("should drop it", () => {
      expect(takeHangoutSignInDraft(EDITOR)).toBeNull();
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });
  });

  describe.each([
    ["is not JSON", "{"],
    ["does not hold a draft", JSON.stringify({ value: { account: OWNER, draft: { name: 1 } }, returnTo: EDITOR, expiresAt: Number.MAX_SAFE_INTEGER })],
    ["holds a draft without its account", JSON.stringify({ value: { draft: emptyDraft() }, returnTo: EDITOR, expiresAt: Number.MAX_SAFE_INTEGER })],
  ])("and the stored value %s", (_, value) => {
    beforeEach(() => { sessionStorage.setItem(KEY, value); });

    it("should give nothing", () => {
      expect(takeHangoutSignInDraft(EDITOR)).toBeNull();
    });
  });

  describe("and the browser refuses storage", () => {
    beforeEach(() => {
      vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("Quota exceeded", "QuotaExceededError"); });
      vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new DOMException("Denied", "SecurityError"); });
    });

    it("should report the draft as not saved and read nothing, without throwing", () => {
      expect(saveHangoutSignInDraft(EDITOR, { account: OWNER, draft })).toBe(false);
      expect(takeHangoutSignInDraft(EDITOR)).toBeNull();
    });
  });

  describe("and the editor follows its draft while sign-in leaves the app", () => {
    let assign: ReturnType<typeof vi.fn>;
    let result: { current: ReturnType<typeof useHangoutSignInDraft> };

    beforeEach(() => {
      assign = vi.fn();
      vi.stubGlobal("location", { origin: "https://example.org", pathname: EDITOR, search: "", hash: "", assign });
      ({ result } = renderHook(() => useHangoutSignInDraft(EDITOR, OWNER, emptyDraft())));
      result.current.remember(draft);
    });

    it("should save it for the account when sign-in returns to this editor", () => {
      openSignIn({ redirectTo: EDITOR });
      expect(takeHangoutSignInDraft(EDITOR)).toEqual({ account: OWNER, draft });
      expect(assign).toHaveBeenCalledOnce();
    });

    it("should save nothing when sign-in returns elsewhere", () => {
      openSignIn({ redirectTo: "/events" });
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });

    it("should save nothing for a draft the writer has not changed", () => {
      result.current.remember(emptyDraft());
      openSignIn({ redirectTo: EDITOR });
      expect(sessionStorage.getItem(KEY)).toBeNull();
      expect(assign).toHaveBeenCalledOnce();
    });

    describe("and the browser refuses to save it", () => {
      beforeEach(() => {
        vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("Quota exceeded", "QuotaExceededError"); });
        act(() => openSignIn({ redirectTo: EDITOR }));
      });

      it("should stop that sign-in and say the draft was not kept", () => {
        expect(assign).not.toHaveBeenCalled();
        expect(result.current.saveFailed).toBe(true);
      });

      it("should let the next sign-in go ahead without the draft", () => {
        openSignIn({ redirectTo: EDITOR });
        expect(assign).toHaveBeenCalledOnce();
      });
    });
  });

  describe("and the draft was submitted", () => {
    let hook: ReturnType<typeof renderHook<ReturnType<typeof useHangoutSignInDraft>, { ended: boolean }>>;

    beforeEach(() => {
      vi.stubGlobal("location", { origin: "https://example.org", pathname: EDITOR, search: "", hash: "", assign: vi.fn() });
      hook = renderHook(({ ended }) => useHangoutSignInDraft(EDITOR, OWNER, emptyDraft(), ended), { initialProps: { ended: false as boolean } });
      hook.result.current.remember(draft);
      hook.result.current.forget();
    });

    it("should keep nothing when the session ends on the success screen", () => {
      hook.rerender({ ended: true });
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });

    it("should keep nothing when sign-in leaves for this editor", () => {
      openSignIn({ redirectTo: EDITOR });
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });

    it("should keep a new draft written after it", () => {
      const next = { ...draft, name: "Second party" };
      hook.result.current.remember(next);
      hook.rerender({ ended: true });
      expect(takeHangoutSignInDraft(EDITOR)).toEqual({ account: OWNER, draft: next });
    });
  });

  describe("and a draft was already kept when the editor submits", () => {
    it("should drop the kept copy", () => {
      saveHangoutSignInDraft(EDITOR, { account: OWNER, draft });
      const { result } = renderHook(() => useHangoutSignInDraft(EDITOR, null, emptyDraft()));
      result.current.forget();
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });
  });

  describe("and a draft kept for one account is read back", () => {
    beforeEach(() => { saveHangoutSignInDraft(EDITOR, { account: OWNER, draft }); });

    it("should offer it to that account", async () => {
      const { result } = renderHook(() => useHangoutSignInDraft(EDITOR, OWNER, emptyDraft()));
      await vi.waitFor(() => expect(result.current.restored).toEqual(draft));
    });

    it("should not offer it to another account, and drop it", async () => {
      const { result } = renderHook(() => useHangoutSignInDraft(EDITOR, OTHER, emptyDraft()));
      await act(async () => {});
      expect(result.current.restored).toBeNull();
      expect(sessionStorage.getItem(KEY)).toBeNull();
    });

    it("should stop offering it once another account signs in, even when the first comes back", async () => {
      const hook = renderHook(({ account }) => useHangoutSignInDraft(EDITOR, account, emptyDraft()), { initialProps: { account: OWNER } });
      await vi.waitFor(() => expect(hook.result.current.restored).toEqual(draft));
      hook.rerender({ account: OTHER });
      expect(hook.result.current.restored).toBeNull();
      hook.rerender({ account: OWNER });
      expect(hook.result.current.restored).toBeNull();
    });
  });
});
