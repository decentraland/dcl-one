import { useContext, type ReactNode } from "react";

import { EventsLocaleContext } from "@dcl/one-ui/apps/sites/events/locale";
import { SitesLocaleContext } from "@ui/web/frames/sites-locale";

export function EventsLocaleProvider({ children }: { children: ReactNode }) {
  const preference = useContext(SitesLocaleContext);
  return (
    <EventsLocaleContext.Provider value={preference?.locale ?? "en"}>
      {children}
    </EventsLocaleContext.Provider>
  );
}
