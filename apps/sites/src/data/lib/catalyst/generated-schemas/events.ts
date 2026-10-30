// GENERATED from library/catalyst-api/events by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { EventAttendeeRecord } from "@catalyst-api/events/EventAttendeeRecord";
import type { EventCategoryRecord } from "@catalyst-api/events/EventCategoryRecord";
import type { EventListData } from "@catalyst-api/events/EventListData";
import type { EventListWithTotal } from "@catalyst-api/events/EventListWithTotal";
import type { EventRecord } from "@catalyst-api/events/EventRecord";
import type { ScheduleRecord } from "@catalyst-api/events/ScheduleRecord";
import type { V1EventRecord } from "@catalyst-api/events/V1EventRecord";

export const EventAttendeeRecordSchema = z.object({
  event_id: z.string(),
  user: z.string(),
  user_name: z.string().nullable(),
  created_at: z.string(),
});

export const EventCategoryRecordSchema = z.object({
  name: z.string(),
  active: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
  i18n: z.record(z.string(), z.unknown()),
});

export const EventRecordSchema = z.object({
  id: z.string(),
  name: z.string(),
  image: z.string().nullable(),
  image_vertical: z.string().nullable(),
  description: z.string().nullable(),
  contact: z.string().optional(),
  details: z.string().optional(),
  start_at: z.string().nullable(),
  finish_at: z.string().nullable(),
  next_start_at: z.string().nullable(),
  next_finish_at: z.string().nullable(),
  duration: z.number().nullable(),
  all_day: z.boolean(),
  x: z.number(),
  y: z.number(),
  server: z.string().nullable(),
  url: z.string().nullable(),
  user: z.string().nullable(),
  user_name: z.string().nullable(),
  estate_id: z.string().nullable(),
  estate_name: z.string().nullable(),
  scene_name: z.string().nullable(),
  approved: z.boolean(),
  rejected: z.boolean(),
  highlighted: z.boolean(),
  trending: z.boolean(),
  world: z.boolean(),
  recurrent: z.boolean(),
  recurrent_frequency: z.string().nullable(),
  recurrent_weekday_mask: z.number(),
  recurrent_month_mask: z.number(),
  recurrent_interval: z.number(),
  recurrent_setpos: z.number().nullable(),
  recurrent_monthday: z.number().nullable(),
  recurrent_count: z.number().nullable(),
  recurrent_until: z.string().nullable(),
  recurrent_dates: z.array(z.string()),
  categories: z.array(z.string()),
  schedules: z.array(z.string()),
  total_attendees: z.number(),
  latest_attendees: z.array(z.string()),
  coordinates: z.tuple([z.number(), z.number()]),
  position: z.tuple([z.number(), z.number()]),
  live: z.boolean(),
  attending: z.boolean(),
  place_id: z.string().nullable(),
  community_id: z.string().nullable(),
  featured_item: z.string().nullable(),
  created_at: z.string().nullable(),
  updated_at: z.string().nullable(),
  approved_by: z.string().nullable(),
  rejected_by: z.string().nullable(),
  rejection_reason: z.string().nullable(),
  deleted_by_user: z.boolean(),
  deleted_by_admin: z.boolean(),
  deleted_by: z.string().nullable(),
  deleted_at: z.string().nullable(),
  deleted_reason: z.string().nullable(),
  previous_place_id: z.string().nullable(),
  connected_addresses: z.array(z.string()).optional(),
});

export const EventListWithTotalSchema = z.object({
  events: z.array(EventRecordSchema),
  total: z.number(),
});

export const EventListDataSchema = z.union([EventListWithTotalSchema, z.array(EventRecordSchema)]);

export const ScheduleRecordSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  image: z.string().nullable(),
  theme: z.string().nullable(),
  background: z.array(z.string()),
  active_since: z.string().nullable(),
  active_until: z.string().nullable(),
  active: z.boolean(),
  created_at: z.string().nullable(),
  updated_at: z.string().nullable(),
});

export const V1EventRecordSchema = z.object({
  destination_id: z.string().nullable(),
  destination: z.record(z.string(), z.unknown()).nullable(),
  id: z.string(),
  name: z.string(),
  image: z.string().nullable(),
  image_vertical: z.string().nullable(),
  description: z.string().nullable(),
  start_at: z.string().nullable(),
  finish_at: z.string().nullable(),
  next_start_at: z.string().nullable(),
  next_finish_at: z.string().nullable(),
  duration: z.number().nullable(),
  all_day: z.boolean(),
  x: z.number(),
  y: z.number(),
  server: z.string().nullable(),
  url: z.string().nullable(),
  user: z.string().nullable(),
  user_name: z.string().nullable(),
  estate_id: z.string().nullable(),
  estate_name: z.string().nullable(),
  scene_name: z.string().nullable(),
  approved: z.boolean(),
  rejected: z.boolean(),
  highlighted: z.boolean(),
  trending: z.boolean(),
  world: z.boolean(),
  recurrent: z.boolean(),
  recurrent_frequency: z.string().nullable(),
  recurrent_weekday_mask: z.number(),
  recurrent_month_mask: z.number(),
  recurrent_interval: z.number(),
  recurrent_setpos: z.number().nullable(),
  recurrent_monthday: z.number().nullable(),
  recurrent_count: z.number().nullable(),
  recurrent_until: z.string().nullable(),
  recurrent_dates: z.array(z.string()),
  categories: z.array(z.string()),
  schedules: z.array(z.string()),
  total_attendees: z.number(),
  latest_attendees: z.array(z.string()),
  coordinates: z.tuple([z.number(), z.number()]),
  position: z.tuple([z.number(), z.number()]),
  live: z.boolean(),
  attending: z.boolean(),
  place_id: z.string().nullable(),
  community_id: z.string().nullable(),
  featured_item: z.string().nullable(),
  created_at: z.string().nullable(),
  updated_at: z.string().nullable(),
  approved_by: z.string().nullable(),
  rejected_by: z.string().nullable(),
  rejection_reason: z.string().nullable(),
  deleted_by_user: z.boolean(),
  deleted_by_admin: z.boolean(),
  deleted_by: z.string().nullable(),
  deleted_at: z.string().nullable(),
  deleted_reason: z.string().nullable(),
  previous_place_id: z.string().nullable(),
  connected_addresses: z.array(z.string()).optional(),
});

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;
type Mutual<A, B> = AssignableTo<A, B> extends true ? AssignableTo<B, A> : false;
type Assert<T extends true> = T;

export type _AssertEventAttendeeRecord = Assert<Mutual<EventAttendeeRecord, z.infer<typeof EventAttendeeRecordSchema>>>;
export type _AssertEventCategoryRecord = Assert<Mutual<EventCategoryRecord, z.infer<typeof EventCategoryRecordSchema>>>;
export type _AssertEventListData = Assert<Mutual<EventListData, z.infer<typeof EventListDataSchema>>>;
export type _AssertEventListWithTotal = Assert<Mutual<EventListWithTotal, z.infer<typeof EventListWithTotalSchema>>>;
export type _AssertEventRecord = Assert<Mutual<EventRecord, z.infer<typeof EventRecordSchema>>>;
export type _AssertScheduleRecord = Assert<Mutual<ScheduleRecord, z.infer<typeof ScheduleRecordSchema>>>;
export type _AssertV1EventRecord = Assert<Mutual<V1EventRecord, z.infer<typeof V1EventRecordSchema>>>;
