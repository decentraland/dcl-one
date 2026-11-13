import { afterEach, describe, expect, it, vi } from "vitest";

import { loadJumpEvent, loadJumpPlace } from "./jump.server";
import { readJumpTarget } from "./jump";

const BASE = "http://node.test";
const DEPLOYER = "0x1111111111111111111111111111111111111111";

type Answer = { status?: number; body: unknown };

function node(answers: Record<string, Answer>) {
  const calls: string[] = [];
  const fetchImpl = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input).replace(BASE, "");
    const call = `${init?.method ?? "GET"} ${decodeURIComponent(url)}${init?.body ? ` ${String(init.body)}` : ""}`;
    calls.push(call);
    const answer = Object.entries(answers).find(([prefix]) => call.startsWith(prefix))?.[1] ?? { status: 404, body: {} };
    return new Response(JSON.stringify(answer.body), { status: answer.status ?? 200, headers: { "content-type": "application/json" } });
  }) as unknown as typeof fetch;
  return { calls, opts: { fetchImpl, base: BASE } };
}

const place = { title: "Genesis Plaza", description: "The plaza", image: "https://img.test/plaza.png", owner: null, contact_name: "Decentraland Foundation", base_position: "0,0", user_count: 7, world: false, world_name: null };
const profile = { avatars: [{ name: "Builder", hasClaimedName: true, ethAddress: DEPLOYER, avatar: { snapshots: { face256: "https://img.test/face.png" } } }] };
const event = {
  id: "e1",
  name: "Building basics",
  user: DEPLOYER,
  user_name: "Host",
  image: null,
  description: "Learn to build",
  next_start_at: "2026-11-10T16:00:00.000Z",
  next_finish_at: "2026-11-10T17:30:00.000Z",
  coordinates: [-150, 95],
  total_attendees: 150,
  live: false,
  world: false,
  server: null,
  scene_name: "School",
};

afterEach(() => vi.restoreAllMocks());

describe("loadJumpPlace", () => {
  it("shows the place at the position, credited to its scene's deployer", async () => {
    const { calls, opts } = node({
      "GET /places/api/places?positions=0,0": { body: { ok: true, data: [place] } },
      "POST /content/entities/active": { body: [{ id: "bafy1" }] },
      "GET /content/deployments/?entityId=bafy1": { body: { deployments: [{ deployedBy: DEPLOYER }] } },
      "POST /lambdas/profiles": { body: [profile] },
    });
    const result = await loadJumpPlace(readJumpTarget(new URLSearchParams("")), opts);
    expect(result).toMatchObject({ card: { title: "Genesis Plaza", userName: "Builder", user: DEPLOYER, userCount: 7, coordinates: [0, 0], foundation: true } });
    expect(calls).toContain('POST /content/entities/active {"pointers":["0,0"]}');
    expect(calls).toContain(`POST /lambdas/profiles {"ids":["${DEPLOYER}"]}`);
  });

  it("falls back to the scene's contact when the deployer has no profile name", async () => {
    const { opts } = node({ "GET /places/api/places": { body: { data: [{ ...place, contact_name: "Pink Oasis" }] } } });
    const result = await loadJumpPlace(readJumpTarget(new URLSearchParams("position=1,95")), opts);
    expect(result).toMatchObject({ card: { userName: "Pink Oasis", user: undefined } });
  });

  it("shows a generic card for an empty parcel", async () => {
    const { opts } = node({ "GET /places/api/places": { body: { data: [] } } });
    const result = await loadJumpPlace(readJumpTarget(new URLSearchParams("position=5,5")), opts);
    expect(result).toMatchObject({ card: { title: "", userName: "Unknown", coordinates: [5, 5], userCount: 0 } });
  });

  it("is invalid for a position that is not one, or when the places API fails", async () => {
    expect(await loadJumpPlace(readJumpTarget(new URLSearchParams("position=abc")), node({}).opts)).toEqual({ invalid: true });
    const { opts } = node({ "GET /places/api/places": { status: 500, body: {} } });
    expect(await loadJumpPlace(readJumpTarget(new URLSearchParams("position=1,1")), opts)).toEqual({ invalid: true });
  });
});

describe("loadJumpEvent", () => {
  it("shows the event by id with its creator's profile", async () => {
    const { calls, opts } = node({
      "GET /events/api/events/e1": { body: { ok: true, data: event } },
      "POST /lambdas/profiles": { body: [profile] },
    });
    const result = await loadJumpEvent(readJumpTarget(new URLSearchParams("id=e1")), opts);
    expect(result).toMatchObject({ deleted: false, card: { type: "event", title: "Building basics", userName: "Builder", coordinates: [-150, 95], totalAttendees: 150, live: false } });
    expect(calls.some((c) => c.startsWith("GET /places/api"))).toBe(false);
  });

  it("takes the first event at the position", async () => {
    const { calls, opts } = node({ "GET /events/api/events?position=0,5": { body: { ok: true, data: [{ ...event, user: null }] } } });
    const result = await loadJumpEvent(readJumpTarget(new URLSearchParams("position=0,5")), opts);
    expect(result).toMatchObject({ card: { title: "Building basics", userName: "Host" } });
    expect(calls).toEqual(["GET /events/api/events?position=0,5"]);
  });

  it("is invalid when the id is unknown or nothing happens at the position", async () => {
    expect(await loadJumpEvent(readJumpTarget(new URLSearchParams("id=missing")), node({}).opts)).toEqual({ invalid: true });
    const { opts } = node({ "GET /events/api/events": { body: { ok: true, data: [] } } });
    expect(await loadJumpEvent(readJumpTarget(new URLSearchParams("position=9,9")), opts)).toEqual({ invalid: true });
  });

  it("shows a deleted event neither live nor ended", async () => {
    const { opts } = node({ "GET /events/api/events/e1": { body: { ok: true, data: { ...event, user: null, live: true, deleted_by_user: true } } } });
    const result = await loadJumpEvent(readJumpTarget(new URLSearchParams("id=e1")), opts);
    expect(result).toMatchObject({ deleted: true, card: { live: false, finishAt: undefined } });
  });
});
