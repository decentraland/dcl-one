import { z } from "zod";

import { getJSON } from "../client";
import type { GetOptions } from "../client";
import { apiOkOf, okDataTotalOf } from "../envelope";
import { ScheduleRecordSchema } from "../generated-schemas/events";
import { warnInvalid } from "../warn";
import type { ScheduleRecord as RsSchedule } from "@catalyst-api/events/ScheduleRecord";
import type { ApiData as RsApiOk } from "@catalyst-api/places/ApiData";

export type Schedule = z.infer<typeof ScheduleRecordSchema>;

function parseSchedule(raw: unknown): Schedule | null {
  const r = ScheduleRecordSchema.safeParse(raw);
  if (r.success) return r.data;
  warnInvalid("Schedule", r.error.issues);
  return null;
}

function parseSchedules(raw: unknown[]): Schedule[] {
  const out: Schedule[] = [];
  for (const row of raw ?? []) {
    const schedule = parseSchedule(row);
    if (schedule) out.push(schedule);
  }
  return out;
}

const ScheduleListEnvelope = okDataTotalOf(z.array(z.unknown()));

export type _DriftScheduleListEnvelope = Assert<
  AssignableTo<RsApiOk<RsSchedule[]>, z.input<typeof ScheduleListEnvelope>>
>;

export type _DriftScheduleDetailEnvelope = Assert<
  AssignableTo<RsApiOk<RsSchedule>, z.input<typeof ScheduleDetailEnvelope>>
>;
const ScheduleDetailEnvelope = apiOkOf(z.unknown());

export async function fetchSchedules(
  opts: GetOptions = {},
): Promise<{ data: Schedule[]; total: number }> {
  const raw = await getJSON<unknown>("/events/api/schedules", opts);
  const env = ScheduleListEnvelope.safeParse(raw);
  if (!env.success) {
    warnInvalid("Schedules envelope", env.error.issues);
    throw new Error("schedules response did not match the events envelope");
  }
  const data = parseSchedules(env.data.data);
  return { data, total: env.data.total ?? data.length };
}

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;

type Assert<T extends true> = T;

export type _DriftSchedule = Assert<AssignableTo<RsSchedule, Schedule>>;
