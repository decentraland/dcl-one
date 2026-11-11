import { queryOptions } from "@tanstack/react-query";

import { withDeadline } from "../request-deadline";

/**
 * The /help service status: the browser pings each public service and reports up or down per service.
 * Client-safe: no server modules.
 */

export type Service = { readonly name: string; readonly url: string };
export type ServiceState = "ok" | "down";
/** `slow` means some services answer and some do not. */
export type OverallState = "ok" | "slow" | "down";
export type ServiceResult = { readonly name: string; readonly state: ServiceState };

/** A service that has not answered by then counts as down. */
export const SERVICE_PING_TIMEOUT_MS = 8_000;
export const SERVICE_STATUS_STALE_MS = 60_000;

/** The services production Sites pings (decentraland/sites src/pages/help/HelpPage.tsx). */
export const DEFAULT_STATUS_SERVICES: readonly Service[] = [
  { name: "Catalyst EC1", url: "https://peer-ec1.decentraland.org/about" },
  { name: "Catalyst Interconnected", url: "https://interconnected.online/about" },
  { name: "Catalyst Melonwave", url: "https://peer.melonwave.com/about" },
  { name: "Catalyst UADevops", url: "https://peer.uadevops.com/about" },
  { name: "Catalyst DCLNodes", url: "https://peer.dclnodes.io/about" },
  { name: "Catalyst AP1", url: "https://peer-ap1.decentraland.org/about" },
  { name: "Catalyst EU1", url: "https://peer-eu1.decentraland.org/about" },
  { name: "Marketplace", url: "https://marketplace-api.decentraland.org/ping" },
  { name: "Chat", url: "https://peer-ec2.decentraland.org/about" },
  { name: "Builder", url: "https://builder-api.decentraland.org/v1/info" },
  { name: "Places", url: "https://places.decentraland.org/api/status" },
  { name: "Events", url: "https://events.decentraland.org/api/status" },
];

export const DEFAULT_STATUS_PAGE_URL = "https://status.decentraland.org/";

function isHttpsUrl(value: unknown): value is string {
  if (typeof value !== "string") return false;
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * The service list a deployment configures as JSON (`[{"name": "...", "url": "https://..."}]`). Anything else,
 * including an empty list, an entry that is not an https URL or two entries with the same name (the name identifies
 * a service in the list), falls back to the defaults.
 */
export function parseStatusServices(raw: string | undefined): readonly Service[] {
  if (!raw || raw.trim() === "") return DEFAULT_STATUS_SERVICES;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return DEFAULT_STATUS_SERVICES;
  }
  if (!Array.isArray(parsed) || parsed.length === 0) return DEFAULT_STATUS_SERVICES;
  const services: Service[] = [];
  for (const entry of parsed) {
    if (!entry || typeof entry !== "object") return DEFAULT_STATUS_SERVICES;
    const { name, url } = entry as { name?: unknown; url?: unknown };
    if (typeof name !== "string" || name.trim() === "" || !isHttpsUrl(url)) return DEFAULT_STATUS_SERVICES;
    if (services.some((service) => service.name === name.trim())) return DEFAULT_STATUS_SERVICES;
    services.push({ name: name.trim(), url });
  }
  return services;
}

/** An https URL, or the default status page. */
export function parseStatusPageUrl(raw: string | undefined): string {
  return isHttpsUrl(raw?.trim()) ? raw!.trim() : DEFAULT_STATUS_PAGE_URL;
}

/**
 * One ping: an OK answer is up; a non-OK answer, a network error or no answer within the timeout is down. The
 * body is never read, so it is cancelled. A caller's abort (the query unmounted) rejects instead of reporting down.
 */
export async function pingService(
  url: string,
  signal?: AbortSignal,
  fetchImpl: typeof fetch = fetch,
  timeoutMs = SERVICE_PING_TIMEOUT_MS,
): Promise<ServiceState> {
  try {
    return await withDeadline(
      async (deadline) => {
        const response = await fetchImpl(url, { signal: deadline, cache: "no-store" });
        await response.body?.cancel().catch(() => undefined);
        return response.ok ? "ok" : "down";
      },
      timeoutMs,
      signal,
    );
  } catch (error) {
    if (signal?.aborted) throw error;
    return "down";
  }
}

export function overallState(results: readonly ServiceResult[]): OverallState {
  if (results.every((r) => r.state === "ok")) return "ok";
  if (results.every((r) => r.state === "down")) return "down";
  return "slow";
}

export const serviceStatusKeys = {
  all: (services: readonly Service[]) =>
    ["service-status", services.map((s) => [s.name, s.url] as const)] as const,
};

export const serviceStatusQuery = (services: readonly Service[], fetchImpl?: typeof fetch) =>
  queryOptions({
    queryKey: serviceStatusKeys.all(services),
    staleTime: SERVICE_STATUS_STALE_MS,
    // One round of pings per visit, as on decentraland.org: returning to the tab does not ping every service again.
    refetchOnWindowFocus: false,
    queryFn: ({ signal }): Promise<ServiceResult[]> =>
      Promise.all(
        services.map(async (service) => ({
          name: service.name,
          state: await pingService(service.url, signal, fetchImpl),
        })),
      ),
  });
