import { describe, expect, it } from "vitest";
import { applyExemptions } from "./schema-honesty-exemptions.mts";

const finding = { file: "data/source.ts", rule: "fabricating-catch", text: 'return "canceled";', line: 10 };
const exemption = { file: finding.file, rule: finding.rule, text: finding.text, why: "The user canceled the picker." };

describe("reviewed schema exemptions", () => {
  it("survives line shifts without granting approval to code at the old line", () => {
    const moved = { ...finding, line: 200 };
    const replacement = { ...finding, text: "return [];" };
    expect(applyExemptions([replacement, moved], [exemption])).toEqual({ violations: [replacement], stale: [] });
  });

  it("rejects changed code, rule, or source file", () => {
    for (const changed of [
      { ...finding, text: 'return "saved";' },
      { ...finding, rule: "fabricated-default" },
      { ...finding, file: "data/another-source.ts" },
    ]) {
      expect(applyExemptions([changed], [exemption])).toEqual({ violations: [changed], stale: [exemption] });
    }
  });

  it("does not ignore literal whitespace or a change after the first 100 characters", () => {
    for (const [before, after] of [
      ['return "not available";', 'return "not  available";'],
      ["x".repeat(110) + "null", "x".repeat(110) + "0"],
    ]) {
      const changed = { ...finding, text: after };
      const reviewed = { ...exemption, text: before };
      expect(applyExemptions([changed], [reviewed]).violations).toEqual([changed]);
    }
  });

  it("counts approvals so a copied default is a new violation", () => {
    const copy = { ...finding, line: 30 };
    expect(applyExemptions([finding, copy], [exemption]).violations).toEqual([copy]);
    expect(applyExemptions([finding, copy], [exemption, exemption])).toEqual({ violations: [], stale: [] });
    expect(applyExemptions([finding], [exemption, exemption]).stale).toEqual([exemption]);
  });

  it("reports approvals for removed code and rejects missing justifications", () => {
    expect(applyExemptions([], [exemption]).stale).toEqual([exemption]);
    expect(() => applyExemptions([finding], [{ ...exemption, why: " " }])).toThrow(/justification/);
  });
});
