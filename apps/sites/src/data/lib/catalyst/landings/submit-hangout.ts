import { z } from "zod";
import { catalystBase, postJSON, signedGetJSON } from "../client";
import { apiOkOf } from "../envelope";
import { EventRecordSchema } from "../generated-schemas/events";
import type { AuthIdentity } from "../../auth/types";
import { signedFetch } from "../../auth/signer";

const HANGOUT_BOUNDS = {
  nameMax: 150,
  descriptionMax: 5000,
  coordMin: -150,
  coordMax: 150,
  maxRecurrent: 10,
  defaultDurationMs: 3_600_000,
} as const;

export type HangoutDraft = {
  name: string;
  description: string;
  startDate: string;
  startTime: string;
  durationHours: number;
  allDay: boolean;
  location: "land" | "world";
  coordX: number;
  coordY: number;
  worldName: string;
  recurrent: boolean;
  recurrence: string;
  recurrentUntil: string;
  recurrentCount?: number | null;
  category: string;
  communityId: string;
  contact: string;
  details: string;
  imagePreviewUrl: string | null;
  verticalImagePreviewUrl: string | null;
};

const RECURRENCE_FREQUENCIES: Record<string, string> = {
  every_hour: "HOURLY", every_day: "DAILY", every_week: "WEEKLY", every_month: "MONTHLY", every_year: "YEARLY",
};

export function emptyDraft(): HangoutDraft {
  return {
    name: "",
    description: "",
    startDate: "",
    startTime: "",
    durationHours: 2,
    allDay: false,
    location: "land",
    coordX: 0,
    coordY: 0,
    worldName: "",
    recurrent: false,
    recurrence: "every_week",
    recurrentUntil: "",
    category: "",
    communityId: "",
    contact: "",
    details: "",
    imagePreviewUrl: null,
    verticalImagePreviewUrl: null,
  };
}

function toIso(date: string, time: string): string | null {
  if (!date) return null;
  const t = time && /^\d{1,2}:\d{2}$/.test(time) ? time : "00:00";
  const d = new Date(`${date}T${t}:00.000Z`);
  return Number.isNaN(d.getTime()) ? null : d.toISOString();
}

export function toCreateEventBody(draft: HangoutDraft, original?: z.infer<typeof EventRecordSchema>) {
  let start = toIso(draft.startDate, draft.startTime);
  if (start && original?.start_at && new Date(original.start_at).toISOString().slice(0, 16) === start.slice(0, 16)) start = original.start_at;
  const frequency = RECURRENCE_FREQUENCIES[draft.recurrence] ?? null;
  const sameRecurrence = original?.recurrent_frequency === frequency;
  const until = draft.recurrent && draft.recurrentUntil
    ? original?.recurrent_until?.slice(0, 10) === draft.recurrentUntil ? original.recurrent_until : `${draft.recurrentUntil}T23:59:59.000Z`
    : null;
  return {
    name: draft.name.trim(),
    description: draft.description.trim(),
    start_at: start,
    duration: (draft.allDay ? 24 : draft.durationHours) * 3_600_000,
    all_day: draft.allDay,
    x: draft.location === "world" ? 0 : draft.coordX,
    y: draft.location === "world" ? 0 : draft.coordY,
    world: draft.location === "world",
    server: draft.location === "world" ? draft.worldName.trim() : null,
    recurrent: draft.recurrent,
    recurrent_frequency: draft.recurrent ? frequency : null,
    recurrent_interval: sameRecurrence ? original?.recurrent_interval ?? 1 : 1,
    recurrent_weekday_mask: sameRecurrence ? original?.recurrent_weekday_mask ?? 0 : 0,
    recurrent_month_mask: sameRecurrence ? original?.recurrent_month_mask ?? 0 : 0,
    recurrent_monthday: sameRecurrence ? original?.recurrent_monthday ?? null : null,
    recurrent_setpos: sameRecurrence ? original?.recurrent_setpos ?? null : null,
    recurrent_count: draft.recurrent ? draft.recurrentCount ?? null : null,
    recurrent_until: until,
    categories: original?.categories[0] === draft.category ? original.categories : draft.category ? [draft.category] : [],
    community_id: draft.communityId.trim() || null,
    image: draft.imagePreviewUrl,
    image_vertical: draft.verticalImagePreviewUrl,
    ...(!original ? { contact: draft.contact.trim() || null, details: draft.details.trim() || null } : {}),
  };
}

const EventEnvelope = apiOkOf(EventRecordSchema);
export type EditableHangout = z.infer<typeof EventRecordSchema>;

export async function loadEditableHangout(identity: AuthIdentity, id: string, signal?: AbortSignal): Promise<EditableHangout> {
  return EventEnvelope.parse(await signedGetJSON(`/events/api/events/${encodeURIComponent(id)}`, { identity, signal })).data;
}

export function eventToDraft(event: EditableHangout): HangoutDraft {
  const start = event.start_at ? new Date(event.start_at).toISOString() : "";
  return {
    ...emptyDraft(), name: event.name, description: event.description ?? "", startDate: start.slice(0, 10), startTime: start.slice(11, 16),
    durationHours: (event.duration ?? 7_200_000) / 3_600_000, allDay: event.all_day, location: event.world ? "world" : "land",
    coordX: event.x, coordY: event.y, worldName: event.server ?? "", recurrent: event.recurrent,
    recurrence: Object.entries(RECURRENCE_FREQUENCIES).find(([, frequency]) => frequency === event.recurrent_frequency)?.[0] ?? event.recurrent_frequency ?? "every_week",
    recurrentUntil: event.recurrent_until?.slice(0, 10) ?? "", recurrentCount: event.recurrent_count, category: event.categories[0] ?? "", communityId: event.community_id ?? "",
    imagePreviewUrl: event.image, verticalImagePreviewUrl: event.image_vertical,
  };
}

export function buildHangoutSubmit(identitySource: () => AuthIdentity | null, original?: EditableHangout): SubmitFn {
  return async ({ draft, signal }) => {
    const identity = identitySource();
    if (!identity) throw new Error("Sign in to submit a hangout.");
    for (const step of ["details", "location", "schedule"]) {
      if (!isStepValid(step, draft)) throw new Error("Complete the hangout details, location and schedule before submitting.");
    }
    const path = original ? `/events/api/events/${encodeURIComponent(original.id)}` : "/events/api/events";
    const response = await postJSON(path, toCreateEventBody(draft, original), { identity, method: original ? "PATCH" : "POST", signal });
    return apiOkOf(SubmitResultSchema).parse(response).data;
  };
}

export async function uploadHangoutPoster(identity: AuthIdentity, file: File, vertical: boolean): Promise<string> {
  const types = vertical ? ["image/jpeg", "image/png", "image/webp"] : ["image/jpeg", "image/png", "image/webp", "image/gif"];
  if (!types.includes(file.type) || file.size === 0 || file.size > 500 * 1024) throw new Error("Choose an image of at most 500 KiB in a supported format.");
  const body = new FormData();
  body.append("poster", file);
  const base = catalystBase();
  const response = await signedFetch(identity, `${base}/events/api/${vertical ? "poster-vertical" : "poster"}`, { method: "POST", body });
  if (!response.ok) throw new Error(`Image upload failed (${response.status}). Please try again.`);
  const Poster = z.object({ url: z.string() });
  const raw: unknown = await response.json();
  const poster = z.union([Poster, apiOkOf(Poster).transform((value) => value.data)]).parse(raw);
  const origin = typeof window === "undefined" ? base : window.location.origin;
  const absoluteBase = new URL(base || origin, origin);
  return new URL(poster.url, absoluteBase).href;
}

type DraftIssues = Partial<Record<keyof HangoutDraft, string>>;

function validateStep(step: string, draft: HangoutDraft): DraftIssues {
  const issues: DraftIssues = {};
  if (step === "details") {
    if (!draft.name.trim()) issues.name = "Hangout name is required";
    else if (draft.name.length > HANGOUT_BOUNDS.nameMax)
      issues.name = `Name must be ${HANGOUT_BOUNDS.nameMax} characters or fewer`;
    if (draft.description.length > HANGOUT_BOUNDS.descriptionMax)
      issues.description = `Description must be ${HANGOUT_BOUNDS.descriptionMax} characters or fewer`;
  }
  if (step === "location") {
    if (draft.location === "world") {
      if (!draft.worldName) issues.worldName = "Select a world";
    } else {
      const { coordMin, coordMax } = HANGOUT_BOUNDS;
      if (draft.coordX < coordMin || draft.coordX > coordMax)
        issues.coordX = `X must be between ${coordMin} and ${coordMax}`;
      if (draft.coordY < coordMin || draft.coordY > coordMax)
        issues.coordY = `Y must be between ${coordMin} and ${coordMax}`;
    }
  }
  if (step === "schedule") {
    if (!draft.startDate) issues.startDate = "Pick a date";
    if (!toIso(draft.startDate, draft.startTime)) issues.startDate = "Pick a valid date";
    if (!Number.isFinite(draft.durationHours) || draft.durationHours <= 0 || draft.durationHours > 24) issues.durationHours = "Duration must be between 0 and 24 hours";
    if (!draft.allDay && !draft.startTime) issues.startTime = "Pick a start time";
    if (draft.recurrent && !draft.recurrentUntil && !draft.recurrentCount)
      issues.recurrentUntil = "Pick an end date for the recurrence";
    if (draft.recurrent && !RECURRENCE_FREQUENCIES[draft.recurrence]) issues.recurrence = "Choose a supported recurrence frequency";
    if (draft.recurrent && draft.recurrentUntil && draft.recurrentUntil < draft.startDate) issues.recurrentUntil = "Recurrence must end on or after the start date";
  }
  return issues;
}

export function isStepValid(step: string, draft: HangoutDraft): boolean {
  return Object.keys(validateStep(step, draft)).length === 0;
}

const SubmitResultSchema = z.object({
  id: z.string(),
  approved: z.boolean(),
});
export type SubmitResult = z.infer<typeof SubmitResultSchema>;

export async function simulateSubmitHangout(
  draft: HangoutDraft,
  opts: { signal?: AbortSignal; delayMs?: number } = {},
): Promise<SubmitResult> {
  const body = toCreateEventBody(draft);
  if (!body.name) throw new Error("name is required");

  await new Promise<void>((resolve, reject) => {
    const t = setTimeout(resolve, opts.delayMs ?? 500);
    opts.signal?.addEventListener("abort", () => {
      clearTimeout(t);
      reject(new Error("aborted"));
    });
  });

  const id = `local-sim-${Math.abs(hashString(body.name + (body.start_at ?? ""))).toString(16).padStart(12, "0").slice(0, 12)}`;
  return { id, approved: false };
}

function hashString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (Math.imul(h, 31) + s.charCodeAt(i)) | 0;
  return h;
}

export type SubmitFn = (args: {
  draft: HangoutDraft;
  signal?: AbortSignal;
}) => Promise<SubmitResult>;
