import { EventsLocaleProvider } from "@/features/events/locale";
import PendingEventsPage from "@/pages/events/PendingEventsPage";

export default function PendingEventsRoute() {
  return <EventsLocaleProvider><PendingEventsPage /></EventsLocaleProvider>;
}
