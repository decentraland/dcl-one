import { queryOptions } from "@tanstack/react-query";

export type ChromeNotificationsFetcher = (
  signal?: AbortSignal,
) => Promise<unknown>;

export const chromeKeys = {
  notifications: (account: string | null) =>
    [
      "private",
      account?.toLowerCase() ?? null,
      "chrome",
      "notifications",
    ] as const,
};

export function chromeNotificationsQuery(
  account: string | null,
  fetcher?: ChromeNotificationsFetcher,
) {
  return queryOptions({
    queryKey: chromeKeys.notifications(account),
    queryFn: ({ signal }) => {
      if (!account || !fetcher)
        throw new Error("Notifications are unavailable.");
      return fetcher(signal);
    },
    retry: false,
  });
}
