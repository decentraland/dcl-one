import type { ScriptColor, ScriptParamColor } from './types';

export function isColorValue(value: unknown): value is ScriptColor {
  if (!value || typeof value !== 'object') return false;
  const { r, g, b } = value as Record<string, unknown>;
  return typeof r === 'number' && typeof g === 'number' && typeof b === 'number';
}

// Shapes any stored or authored colour to the declared kind: a Color3 drops `a`, a Color4
// always carries one (opaque unless the value says otherwise).
export function toColorOfKind(value: ScriptColor, alpha: boolean): ScriptColor {
  const { r, g, b } = value;
  if (!alpha) return { r, g, b };
  return { r, g, b, a: typeof value.a === 'number' ? value.a : 1 };
}

// Keeps a stored colour if it still is one, reshaped to the declared kind (a script that changes
// a param from Color3 to Color4 keeps the creator's channels and gains an opaque alpha).
export function reconcileColor(sourceParam: ScriptParamColor, storedValue: unknown): ScriptColor {
  return toColorOfKind(
    isColorValue(storedValue) ? storedValue : sourceParam.value,
    sourceParam.value.a !== undefined,
  );
}
