import { EventsLocaleProvider } from "@/features/events/locale";
import UsersAdminPage from "@/pages/events/UsersAdminPage";

export default function EventUsersRoute() {
  return <EventsLocaleProvider><UsersAdminPage /></EventsLocaleProvider>;
}
