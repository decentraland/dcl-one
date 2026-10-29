import { afterEach, vi } from "vitest";

afterEach(() => {
  vi.useRealTimers();
});

// Simulated latency in these machines runs on vi.useFakeTimers(); settle()
// advances it so the pending promise resolves.
export async function settle<T>(pending: Promise<T>): Promise<T> {
  await vi.runAllTimersAsync();
  return pending;
}
