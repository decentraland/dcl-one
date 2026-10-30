import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), postJSON: vi.fn() }));
vi.mock("../../auth/signer", () => ({ signedFetch: vi.fn() }));
import { postJSON } from "../client";
import { signedFetch } from "../../auth/signer";
import { buildHangoutSubmit, emptyDraft as emptyHangout, eventToDraft, uploadHangoutPoster, type EditableHangout, type HangoutDraft } from "./submit-hangout";
import { buildScheduleSubmit, emptyDraft as emptySchedule, type ScheduleDraft } from "./schedules";
import type { AuthIdentity } from "../../auth/types";

describe("when submitting a hangout", () => {
  let identity: AuthIdentity | null;
  let draft: HangoutDraft;
  let submit: ReturnType<typeof buildHangoutSubmit>;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    draft = { ...emptyHangout(), name: "World music", startDate: "2099-06-01", startTime: "18:00", durationHours: 2, location: "world", worldName: "music.dcl.eth", recurrent: true, recurrence: "every_week", recurrentUntil: "2099-07-01", category: "music" };
    submit = buildHangoutSubmit(() => identity);
    vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { id: "saved-event", approved: false } });
  });

  afterEach(() => vi.resetAllMocks());

  it("should persist the world, duration and recurrence using the signed legacy event endpoint", async () => {
    await submit({ draft });
    expect(postJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ name: "World music", world: true, server: "music.dcl.eth", start_at: "2099-06-01T18:00:00.000Z", duration: 7_200_000, recurrent_frequency: "WEEKLY", recurrent_until: "2099-07-01T23:59:59.000Z", categories: ["music"] }), expect.objectContaining({ identity, method: "POST" }));
  });

  it("should return the saved event id from the server", async () => {
    await expect(submit({ draft })).resolves.toEqual({ id: "saved-event", approved: false });
  });

  it("should require a current identity when the user signs out before submitting", async () => {
    identity = null;
    await expect(submit({ draft })).rejects.toThrow("Sign in to submit a hangout.");
    expect(postJSON).not.toHaveBeenCalled();
  });

  it("should reject invalid form dates before sending a request", async () => {
    draft.startDate = "";
    await expect(submit({ draft })).rejects.toThrow("Complete the hangout details");
    expect(postJSON).not.toHaveBeenCalled();
  });

  describe("and editing an existing recurrence", () => {
    let original: EditableHangout;
    beforeEach(() => {
      original = {
        id: "existing-event", name: "Monthly series", image: "https://example.com/cover.png", image_vertical: null,
        description: "Series", start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T20:00:00Z", next_start_at: null, next_finish_at: null,
        duration: 7_200_000, all_day: false, x: 0, y: 0, server: "music.dcl.eth", url: null, user: identity?.signer ?? "", user_name: null,
        estate_id: null, estate_name: null, scene_name: null, approved: true, rejected: false, highlighted: false, trending: false, world: true,
        recurrent: true, recurrent_frequency: "MONTHLY", recurrent_weekday_mask: 2, recurrent_month_mask: 4, recurrent_interval: 2,
        recurrent_setpos: -1, recurrent_monthday: null, recurrent_count: 8, recurrent_until: null, recurrent_dates: [], categories: ["music", "social"], schedules: [],
        total_attendees: 0, latest_attendees: [], coordinates: [0, 0], position: [0, 0], live: false, attending: false, place_id: null, community_id: "community",
        featured_item: null, created_at: null, updated_at: null, approved_by: null, rejected_by: null, rejection_reason: null,
        deleted_by_user: false, deleted_by_admin: false, deleted_by: null, deleted_at: null, deleted_reason: null, previous_place_id: null,
      };
      draft = eventToDraft(original);
      submit = buildHangoutSubmit(() => identity, original);
    });

    it("should preserve recurrence selectors and multiple categories in the event patch", async () => {
      await submit({ draft });
      expect(postJSON).toHaveBeenCalledWith("/events/api/events/existing-event", expect.objectContaining({ recurrent_frequency: "MONTHLY", recurrent_interval: 2, recurrent_weekday_mask: 2, recurrent_month_mask: 4, recurrent_setpos: -1, recurrent_count: 8, categories: ["music", "social"] }), expect.objectContaining({ method: "PATCH" }));
    });

    it("should omit private contact and moderator details from an edit", async () => {
      draft.contact = "different@example.com";
      draft.details = "Replacement";
      await submit({ draft });
      expect(vi.mocked(postJSON).mock.calls[0]?.[1]).not.toHaveProperty("contact");
      expect(vi.mocked(postJSON).mock.calls[0]?.[1]).not.toHaveProperty("details");
    });

    it("should use the recurrence count in the submitted draft", async () => {
      draft.recurrentCount = 5;
      await submit({ draft });
      expect(postJSON).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({ recurrent_count: 5 }), expect.any(Object));
    });

    describe("and the series repeats yearly", () => {
      beforeEach(() => { original.recurrent_frequency = "YEARLY"; draft = eventToDraft(original); });
      it("should preserve the yearly frequency on an unrelated edit", async () => {
        await submit({ draft });
        expect(postJSON).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({ recurrent_frequency: "YEARLY", recurrent_interval: 2 }), expect.any(Object));
      });
    });

    describe("and the series repeats hourly", () => {
      beforeEach(() => { original.recurrent_frequency = "HOURLY"; draft = eventToDraft(original); });
      it("should preserve the hourly frequency on an unrelated edit", async () => {
        await submit({ draft });
        expect(postJSON).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({ recurrent_frequency: "HOURLY", recurrent_interval: 2 }), expect.any(Object));
      });
    });
  });
});

describe("when saving a moderator schedule", () => {
  let identity: AuthIdentity;
  let draft: ScheduleDraft;
  let submit: ReturnType<typeof buildScheduleSubmit>;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    draft = { ...emptySchedule(), name: "Festival", activeSinceDate: "2099-06-01", activeUntilDate: "2099-06-03" };
    submit = buildScheduleSubmit(() => identity);
    vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { id: "saved-schedule", active: true } });
  });

  afterEach(() => { vi.resetAllMocks(); vi.unstubAllGlobals(); });

  it("should use RFC3339 boundaries with the signed legacy schedule endpoint", async () => {
    await submit({ draft });
    expect(postJSON).toHaveBeenCalledWith("/events/api/schedules", expect.objectContaining({ active_since: "2099-06-01T00:00:00.000Z", active_until: "2099-06-03T23:59:59.000Z" }), expect.objectContaining({ identity, method: "POST" }));
  });

  it("should patch the selected schedule instead of creating another", async () => {
    await submit({ draft, scheduleId: "saved-schedule" });
    expect(postJSON).toHaveBeenCalledWith("/events/api/schedules/saved-schedule", expect.any(Object), expect.objectContaining({ method: "PATCH" }));
  });

  it("should surface permission failures from the server", async () => {
    vi.mocked(postJSON).mockRejectedValueOnce(new Error("Moderator access required"));
    await expect(submit({ draft })).rejects.toThrow("Moderator access required");
  });
});

describe("when uploading event covers", () => {
  let identity: AuthIdentity;
  let file: File;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    file = new File([new Uint8Array([137, 80, 78, 71])], "cover.png", { type: "image/png" });
    vi.mocked(signedFetch).mockResolvedValue(new Response(JSON.stringify({ filename: "poster/cover.png", url: "/poster/cover.png", size: file.size, type: "image/png" })));
  });

  afterEach(() => vi.resetAllMocks());

  it("should send multipart image data using the signed poster endpoint", async () => {
    await uploadHangoutPoster(identity, file, false);
    expect(signedFetch).toHaveBeenCalledWith(identity, expect.stringContaining("/events/api/poster"), { method: "POST", body: expect.any(FormData) });
  });

  it("should accept the server bare poster response and return its reachable URL", async () => {
    await expect(uploadHangoutPoster(identity, file, true)).resolves.toMatch(/\/poster\/cover\.png$/);
  });

  it("should reject oversized images before uploading", async () => {
    file = new File([new Uint8Array(500 * 1024 + 1)], "large.png", { type: "image/png" });
    await expect(uploadHangoutPoster(identity, file, false)).rejects.toThrow("at most 500 KiB");
    expect(signedFetch).not.toHaveBeenCalled();
  });

  describe("and the browser uses a relative catalyst base", () => {
    beforeEach(() => vi.stubGlobal("window", { __CATALYST_BASE__: "", location: { origin: "https://discovery.example" } }));
    afterEach(() => vi.unstubAllGlobals());
    it("should resolve the uploaded poster to the browser origin", async () => {
      await expect(uploadHangoutPoster(identity, file, false)).resolves.toBe("https://discovery.example/poster/cover.png");
    });
  });
});
