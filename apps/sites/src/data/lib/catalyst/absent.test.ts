import { beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";

import { absentWhenNull } from "./absent";

describe("when parsing a field declared with absentWhenNull", () => {
  let schema: z.ZodType;
  beforeEach(() => {
    schema = z.object({ country: absentWhenNull(z.string()) });
  });

  describe("and the value is an explicit null", () => {
    let input: Record<string, unknown>;
    beforeEach(() => {
      input = { country: null };
    });

    it("should parse the field as undefined", () => {
      expect(schema.parse(input)).toEqual({ country: undefined });
    });
  });

  describe("and the key is missing", () => {
    let input: Record<string, unknown>;
    beforeEach(() => {
      input = {};
    });

    it("should parse the field as undefined", () => {
      expect(schema.parse(input)).toEqual({});
    });
  });

  describe("and the value has the declared type", () => {
    let input: Record<string, unknown>;
    beforeEach(() => {
      input = { country: "Exampleland" };
    });

    it("should keep the value", () => {
      expect(schema.parse(input)).toEqual({ country: "Exampleland" });
    });
  });

  describe("and the value has the wrong type", () => {
    let input: Record<string, unknown>;
    beforeEach(() => {
      input = { country: 42 };
    });

    it("should reject it with an invalid type issue at that field", () => {
      expect(schema.safeParse(input).error?.issues).toEqual([
        expect.objectContaining({ code: "invalid_type", path: ["country"] }),
      ]);
    });
  });
});
