import { EventsLocaleProvider } from "@/features/events/locale";
import PendingEventsPage from "@/features/explorer/event-management/PendingEventsPage";

export default function PendingEventsRoute() {
  return <EventsLocaleProvider><PendingEventsPage /></EventsLocaleProvider>;
}
