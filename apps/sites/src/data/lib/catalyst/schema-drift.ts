import { track } from "@/core/lib/telemetry/track";

// Hydration installs this reporter on every route. Reporting validation failures
// must not pull the place/category parsers into unrelated pages.
export function reportSchemaDrift(kind: string, issues: readonly unknown[]): void {
  console.warn(`[catalyst] ${kind} failed schema validation`, issues);
  try {
    track(
      "catalyst_schema_drift",
      { kind, issues_count: issues.length },
      { sid: "catalyst-schema-guard" },
    );
  } catch {
  }
}
