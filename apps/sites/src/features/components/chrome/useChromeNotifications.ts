import { useQuery } from "@tanstack/react-query";
import { parseNotificationsLoose } from "@ui/data/catalyst/notificationsView";
import {
  chromeNotificationsQuery,
  type ChromeNotificationsFetcher,
} from "@/data/lib/chrome";

export function useChromeNotifications(
  account: string | null,
  open: boolean,
  fetcher?: ChromeNotificationsFetcher,
  preload = false,
) {
  return useQuery({
    ...chromeNotificationsQuery(account, fetcher),
    enabled: !!account && (open || (preload && !!fetcher)),
    staleTime: preload ? 30_000 : 0,
    select: parseNotificationsLoose,
  });
}
