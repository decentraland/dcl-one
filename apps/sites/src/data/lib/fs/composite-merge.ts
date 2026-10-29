type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type ObjectValue = { [key: string]: Json };

const object = (value: Json | undefined): value is ObjectValue => value !== null && typeof value === "object" && !Array.isArray(value);
const canonical = (value: Json | undefined): string | undefined => object(value)
  ? JSON.stringify(Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])))
  : Array.isArray(value) ? JSON.stringify(value.map(canonical)) : JSON.stringify(value);
const same = (a: Json | undefined, b: Json | undefined) => canonical(a) === canonical(b);

export class CompositeConflict extends Error {
  constructor(readonly paths: string[]) {
    super(`Another editor changed the same scene fields: ${paths.slice(0, 5).join(", ")}. Your changes are still in the editor. Reconcile those fields or reopen the saved scene before saving again.`);
    this.name = "CompositeConflict";
  }
}

function parse(text: string): ObjectValue {
  const value = JSON.parse(text) as Json;
  if (!object(value) || value.version !== 1 || !Array.isArray(value.components)) throw new Error("Cannot merge an unsupported scene composite.");
  const components: ObjectValue = {};
  for (const component of value.components) {
    if (!object(component) || typeof component.name !== "string" || !object(component.data) || Object.hasOwn(components, component.name)) {
      throw new Error("Cannot merge a scene with invalid or duplicate components.");
    }
    Object.defineProperty(components, component.name, { value: component, enumerable: true });
  }
  return { ...value, components: Object.fromEntries(Object.entries(components).filter(([, component]) => Object.keys((component as ObjectValue).data as ObjectValue).length > 0)) };
}

function entities(composite: ObjectValue): ObjectValue {
  const result: ObjectValue = Object.create(null);
  for (const [name, component] of Object.entries(composite.components as ObjectValue)) {
    for (const [id, value] of Object.entries((component as ObjectValue).data as ObjectValue)) {
      const entity = (result[id] ??= {}) as ObjectValue;
      Object.defineProperty(entity, name, { value, enumerable: true });
    }
  }
  return result;
}

export function sameSceneComposite(left: string, right: string): boolean {
  return same(parse(left), parse(right));
}

/** Merge authored fields, not engine clocks. Lists are atomic: guessing an order can
 * silently rewire actions or animations. Concurrent allocations of one entity ID
 * are conflicts, never two accidentally combined objects. */
export function mergeSceneComposites(baseline: string, local: string, remote: string): string {
  const base = parse(baseline), ours = parse(local), theirs = parse(remote);
  const conflicts: string[] = [];
  const baseEntities = entities(base), localEntities = entities(ours), remoteEntities = entities(theirs);
  for (const id of Object.keys(localEntities)) {
    if (!Object.hasOwn(baseEntities, id) && Object.hasOwn(remoteEntities, id) && !same(localEntities[id], remoteEntities[id])) conflicts.push(`entities/${id}`);
  }
  const merge = (b: Json | undefined, l: Json | undefined, r: Json | undefined, path: string): Json | undefined => {
    if (same(l, b)) return r;
    if (same(r, b) || same(l, r)) return l;
    if (object(l) && object(r) && (object(b) || b === undefined)) {
      const value: ObjectValue = {};
      for (const key of new Set([...Object.keys(b ?? {}), ...Object.keys(l), ...Object.keys(r)])) {
        const next = merge(b?.[key], l[key], r[key], `${path}/${key}`);
        if (next !== undefined) Object.defineProperty(value, key, { value: next, enumerable: true });
      }
      return value;
    }
    conflicts.push(path);
    return l;
  };
  const result = merge(base, ours, theirs, "scene") as ObjectValue;
  if (conflicts.length) throw new CompositeConflict(conflicts);
  return JSON.stringify({ ...result, components: Object.values(result.components as ObjectValue) });
}
