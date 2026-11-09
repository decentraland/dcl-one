export const HTTP_PROBE_TIMEOUT_MS = 2000;

export async function fetchGeneratesNoServerError(url: string, timeout = HTTP_PROBE_TIMEOUT_MS): Promise<boolean> {
  const timeoutMs = Math.max(1, Math.floor(timeout));
  const response = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
  try {
    return response.status < 500;
  } finally {
    await response.body?.cancel();
  }
}
