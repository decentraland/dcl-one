import { it, expect } from "vitest";

import { describeRequiringPg } from "./require-dep";

import type { DehydratedState } from "@tanstack/react-query";

import { placesFromDehydrated } from "../../src/data/lib/catalyst/places/discover";
import { loader } from "../../src/routes/routes/places";

const d = describeRequiringPg();

const GENESIS_PLAZA = "3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d";

async function runLoader(url: string) {
  const request = new Request(url);
  const result: unknown = await loader({
    request,
    params: {},
    context: {},
  } as never);
  if (result && typeof result === "object" && "data" in (result as object)) {
    return (result as { data: unknown }).data as Record<string, unknown>;
  }
  return result as Record<string, unknown>;
}

function placesOf(payload: Record<string, unknown>) {
  return placesFromDehydrated(payload.dehydratedState as DehydratedState);
}

d("Places route loader (SSR, temp postgres)", () => {
  it("returns the seeded places with no filters", async () => {
    const payload = await runLoader("http://localhost/places");
    const places = placesOf(payload);

    expect(places.length).toBeGreaterThan(0);
    expect(places.map((p) => p.id)).toContain(GENESIS_PLAZA);

    expect(typeof payload.sid).toBe("string");
    expect((payload.sid as string).length).toBeGreaterThan(0);
  });

  it("honors ?category= (server-side filter)", async () => {
    const payload = await runLoader("http://localhost/places?category=music");
    const places = placesOf(payload);
    expect(places.length).toBeGreaterThan(0);
    for (const p of places) expect(p.categories).toContain("music");
  });

  it("honors ?search= (FTS/ILIKE) and surfaces the match", async () => {
    const payload = await runLoader("http://localhost/places?search=plaza");
    const places = placesOf(payload);
    expect(places.map((p) => p.id)).toContain(GENESIS_PLAZA);
  });
});
