import { z } from "zod";

/**
 * A field the upstream may omit or send as an explicit `null` (the lambdas
 * Avatar marks its unset profile fields `nullable`). Both parse to `undefined`,
 * which keeps the field's existing optional type for callers; a present value
 * of the wrong type still fails the parse.
 */
export const absentWhenNull = <T extends z.ZodType>(schema: T) =>
  schema
    .nullish()
    .transform((v) => v ?? undefined)
    .optional();
