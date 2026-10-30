export type SchemaFinding = { file: string; rule: string; text: string };
export type SchemaExemption = SchemaFinding & { why: string };

// Match reviewed code, independent of line shifts. Each entry excuses exactly
// one occurrence, so copying an approved default still requires another review.
export function applyExemptions<T extends SchemaFinding>(findings: T[], exemptions: SchemaExemption[]) {
  const key = (row: SchemaFinding) => JSON.stringify([row.file, row.rule, row.text]);
  const remaining = new Map<string, SchemaExemption[]>();
  for (const exemption of exemptions) {
    if (!exemption.file?.trim() || !exemption.rule?.trim() || !exemption.text?.trim() || !exemption.why?.trim()) {
      throw new Error("Schema exemptions require a file, rule, exact code, and justification");
    }
    const id = key(exemption);
    const rows = remaining.get(id) ?? [];
    rows.push(exemption);
    remaining.set(id, rows);
  }
  const violations = findings.filter(finding => !remaining.get(key(finding))?.pop());
  return { violations, stale: [...remaining.values()].flat() };
}
