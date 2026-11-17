import { Link } from "react-router";

import { Tabs, TabsBar, type TabsLinkProps } from "@dcl/one-ui";

export type PlacesTab = "places" | "communities";

const TABS: readonly { id: PlacesTab; label: string; href: string }[] = [
  { id: "places", label: "Places", href: "/places" },
  { id: "communities", label: "Communities", href: "/places/communities" },
];

/** Loads the other tab's code on hover or focus, as the app's other in-app links do. */
function TabLink({ href, ...props }: TabsLinkProps) {
  return <Link to={href} prefetch="intent" {...props} />;
}

/** The Places and Communities tabs over both /places pages; each tab is its own route. */
export function PlacesTabs({ active }: { active: PlacesTab }) {
  return (
    <TabsBar label="Places sections" inset="content" scrollable>
      {TABS.map((tab) => (
        <Tabs key={tab.id} label={tab.label} href={tab.href} linkComponent={TabLink} selected={tab.id === active} />
      ))}
    </TabsBar>
  );
}
