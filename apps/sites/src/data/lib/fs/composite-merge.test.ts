import { describe, expect, it } from "vitest";
import { CompositeConflict, mergeSceneComposites } from "./composite-merge";

const composite = (data: Record<string, unknown>) => JSON.stringify({ version: 1, components: Object.entries(data).map(([name, data]) => ({ name, data })) });
const transform = (x: number, y = 0) => ({ json: { position: { x, y, z: 0 }, parent: 0 } });
const merged = (base: string, ours: string, theirs: string) => Object.fromEntries(JSON.parse(mergeSceneComposites(base, ours, theirs)).components.map((c: { name: string; data: unknown }) => [c.name, c.data]));

describe("shared scene three-way merge", () => {
  it("retains changes to different entities and different fields of one component", () => {
    const base = composite({ Transform: { 512: transform(0), 513: transform(1) }, Name: { 512: { json: { value: "box" } } } });
    const ours = composite({ Transform: { 512: transform(4), 513: transform(1) }, Name: { 512: { json: { value: "mine" } } } });
    const theirs = composite({ Name: { 512: { json: { value: "box" } } }, Transform: { 512: transform(0, 8), 513: transform(9) } });
    expect(merged(base, ours, theirs)).toEqual({ Transform: { 512: transform(4, 8), 513: transform(9) }, Name: { 512: { json: { value: "mine" } } } });
  });

  it("does not conflict on reordered properties or identical concurrent edits", () => {
    const base = composite({ Name: { 512: { json: { value: "old" } } } });
    const next = composite({ Name: { 512: { json: { value: "new" } } } });
    expect(merged(base, next, next)).toEqual({ Name: { 512: { json: { value: "new" } } } });
  });

  it("does not mistake omitted unused component definitions for deletion of another editor's addition", () => {
    const base = composite({ Name: { 512: { json: { value: "box" } } }, AudioSource: {} });
    const ours = composite({ Name: { 512: { json: { value: "renamed" } } } });
    const theirs = composite({ Name: { 512: { json: { value: "box" } } }, AudioSource: { 512: { json: { audioClipUrl: "tone.wav" } } } });
    expect(merged(base, ours, theirs)).toEqual({ Name: { 512: { json: { value: "renamed" } } }, AudioSource: { 512: { json: { audioClipUrl: "tone.wav" } } } });
  });

  it("preserves independent additions and deletions", () => {
    const base = composite({ Transform: { 512: transform(0), 513: transform(1) } });
    expect(merged(base, composite({ Transform: { 513: transform(1), 600: transform(2) } }), composite({ Transform: { 512: transform(0), 513: transform(1), 601: transform(3) } })))
      .toEqual({ Transform: { 513: transform(1), 600: transform(2), 601: transform(3) } });
  });

  it("rejects same-field edits, delete-versus-edit and conflicting allocation of one entity ID", () => {
    const base = composite({ Transform: { 512: transform(0) } });
    for (const [ours, theirs] of [
      [composite({ Transform: { 512: transform(1) } }), composite({ Transform: { 512: transform(2) } })],
      [composite({ Transform: {} }), composite({ Transform: { 512: transform(2) } })],
      [composite({ Transform: { 512: transform(0), 600: transform(1) } }), composite({ Transform: { 512: transform(0), 600: transform(2) } })],
    ]) expect(() => mergeSceneComposites(base, ours!, theirs!)).toThrow(CompositeConflict);
  });

  it("treats animation/interaction arrays atomically rather than guessing which entry matches", () => {
    const scene = (clip: string) => composite({ Animator: { 512: { json: { states: [{ clip, playing: true }] } } } });
    expect(() => mergeSceneComposites(scene("idle"), scene("walk"), scene("jump"))).toThrow("states");
  });

  it("rejects malformed or duplicate component blocks and unsupported versions", () => {
    const base = composite({ Transform: {} });
    for (const value of ['{}', '{"version":2,"components":[]}', '{"version":1,"components":[{"name":"a","data":{}},{"name":"a","data":{}}]}']) {
      expect(() => mergeSceneComposites(base, value, base)).toThrow();
    }
  });
});
