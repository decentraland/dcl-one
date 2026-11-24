import { useContext } from "react";
import { ChromeLink, useSiteLayout } from "@dcl/one-ui";
import { ProfileNavigationContext, type ProfileNavigationProps } from "@dcl/one-ui/apps/sites";
import "./profilenavigation.css";


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
