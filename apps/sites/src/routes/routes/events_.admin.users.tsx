import { EventsLocaleProvider } from "@/features/events/locale";
import UsersAdminPage from "@/features/explorer/event-management/UsersAdminPage";

export default function EventUsersRoute() {
  return <EventsLocaleProvider><UsersAdminPage /></EventsLocaleProvider>;
}
