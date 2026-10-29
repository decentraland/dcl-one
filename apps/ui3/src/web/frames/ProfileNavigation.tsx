import { createContext, useContext, type ReactNode } from "react";
import ChromeLink from "./ChromeLink";
import "./profilenavigation.css";
import { useSiteLayout } from "./SiteLayout";

type ProfileTab = { id: string; label: ReactNode; href?: string };
type ProfileNavigationProps = {
  tabs: readonly ProfileTab[];
  active?: string;
  onTab?: (id: string) => void;
  label?: string;
};

// The route supplies real destinations once for the shared secondary navigation.
export const ProfileNavigationContext = createContext<ProfileNavigationProps | null>(null);

export default function ProfileNavigation(props: ProfileNavigationProps) {
  const route = useContext(ProfileNavigationContext);
  const layout = useSiteLayout();
  if (route && layout?.sections) return null;
  const { tabs, active, onTab, label = "Profile sections" } = route ?? props;
  return (
    <nav className="profile-nav" aria-label={label}>
      {tabs.map((tab) => {
        const shared = {
          className: "profile-nav__tab" + (tab.id === active ? " is-active" : ""),
          "aria-current": tab.id === active ? "page" as const : undefined,
          onClick: () => onTab?.(tab.id),
        };
        return tab.href ? (
          <ChromeLink key={tab.id} href={tab.href} {...shared}>{tab.label}</ChromeLink>
        ) : (
          <button key={tab.id} type="button" {...shared}>{tab.label}</button>
        );
      })}
    </nav>
  );
}
