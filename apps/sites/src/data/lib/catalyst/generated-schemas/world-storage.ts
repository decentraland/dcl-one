// GENERATED from library/catalyst-api/world-storage by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { KeyListResponse } from "@catalyst-api/world-storage/KeyListResponse";
import type { PaginationInfo } from "@catalyst-api/world-storage/PaginationInfo";
import type { StorageValueRow } from "@catalyst-api/world-storage/StorageValueRow";
import type { UsageResponse } from "@catalyst-api/world-storage/UsageResponse";
import type { ValuesListResponse } from "@catalyst-api/world-storage/ValuesListResponse";

export const PaginationInfoSchema = z.object({
  limit: z.number(),
  offset: z.number(),
  total: z.number(),
});

export const KeyListResponseSchema = z.object({
  data: z.array(z.string()),
  pagination: PaginationInfoSchema,
});

export const StorageValueRowSchema = z.object({
  key: z.string(),
  value: z.unknown(),
});

export const UsageResponseSchema = z.object({
  usedBytes: z.number(),
  maxTotalSizeBytes: z.number(),
});

export const ValuesListResponseSchema = z.object({
  data: z.array(StorageValueRowSchema),
  pagination: PaginationInfoSchema,
});

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;
type Mutual<A, B> = AssignableTo<A, B> extends true ? AssignableTo<B, A> : false;
type Assert<T extends true> = T;

export type _AssertKeyListResponse = Assert<Mutual<KeyListResponse, z.infer<typeof KeyListResponseSchema>>>;
export type _AssertPaginationInfo = Assert<Mutual<PaginationInfo, z.infer<typeof PaginationInfoSchema>>>;
export type _AssertStorageValueRow = Assert<Mutual<StorageValueRow, z.infer<typeof StorageValueRowSchema>>>;
export type _AssertUsageResponse = Assert<Mutual<UsageResponse, z.infer<typeof UsageResponseSchema>>>;
export type _AssertValuesListResponse = Assert<Mutual<ValuesListResponse, z.infer<typeof ValuesListResponseSchema>>>;
