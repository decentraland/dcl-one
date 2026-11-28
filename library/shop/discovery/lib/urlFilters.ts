// from: shop@94584a5c:app/src/hooks/useUrlFilters.ts

export type FilterValue = string | string[] | boolean | null;
export type FilterParams = { get(name: string): string | null };

function decode(raw: string | null, fallback: FilterValue): FilterValue {
  if (raw === null) return fallback;
  if (Array.isArray(fallback)) return raw ? raw.split(",").filter(Boolean) : [];
  if (typeof fallback === "boolean") return raw === "true";
  return raw;
}

export function readUrlFilters<T extends Record<string, FilterValue>>(searchParams: FilterParams, defaults: T): T {
  const out = { ...defaults };
  for (const key of Object.keys(defaults) as (keyof T)[]) out[key] = decode(searchParams.get(String(key)), defaults[key] as FilterValue) as T[keyof T];
  return out;
}
