export function coarseDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  const units: [number, string][] = [
    [86_400, "day"],
    [3_600, "hour"],
    [60, "minute"],
  ];
  for (const [span, name] of units) {
    if (s >= span) {
      const n = Math.floor(s / span);
      return `${n} ${name}${n === 1 ? "" : "s"}`;
    }
  }
  return `${s} second${s === 1 ? "" : "s"}`;
}
