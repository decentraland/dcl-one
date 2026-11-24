import { useContext, type ReactNode } from "react";

import { EventsLocaleContext, sitesFooterCopy, SitesLocaleContext } from "@dcl/one-ui/apps/sites";

export function useEventsFooterCopy() {
  return sitesFooterCopy[useContext(EventsLocaleContext)];
}

export function EventsLocaleProvider({ children }: { children: ReactNode }) {
  const preference = useContext(SitesLocaleContext);
  return (
    <EventsLocaleContext.Provider value={preference?.locale ?? "en"}>
      {children}
    </EventsLocaleContext.Provider>
  );
}
