import { z } from "zod";

import { DataTotalSchema } from "./generated-schemas/market";
import { ApiDataSchema } from "./generated-schemas/places";

export const dataTotalOf = DataTotalSchema;

export const apiOkOf = ApiDataSchema;

export const dataOf = <T extends z.ZodTypeAny>(inner: T) =>
  z.object({ data: inner });

export const okDataTotalOf = <T extends z.ZodTypeAny>(inner: T) =>
  ApiDataSchema(inner).extend({ total: z.number().optional() });
