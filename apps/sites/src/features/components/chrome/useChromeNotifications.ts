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
) {
  return useQuery({
    ...chromeNotificationsQuery(account, fetcher),
    enabled: !!account && open,
    select: parseNotificationsLoose,
  });
}
