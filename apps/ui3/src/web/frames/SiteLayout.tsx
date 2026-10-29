import { createContext, useContext, type ReactNode } from "react";
import "./sitelayout.css";

type SiteLayoutNavigation = { sections?: (closeMenu: () => void) => ReactNode };
const SiteLayoutContext = createContext<SiteLayoutNavigation | null>(null);
export const useSiteLayout = () => useContext(SiteLayoutContext);

export default function SiteLayout({ navigation, sections, children, className = "" }: SiteLayoutNavigation & {
  navigation?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <SiteLayoutContext.Provider value={{ sections }}>
      <div className={"site-layout ui2 " + className + (!sections ? " site-layout--no-sidebar" : "")}>
        {navigation ? <div className="site-layout__navigation">{navigation}</div> : null}
        {sections ? <aside className="site-layout__sidebar" aria-label="Secondary navigation">{sections(() => {})}</aside> : null}
        <div className="site-layout__content">{children}</div>
      </div>
    </SiteLayoutContext.Provider>
  );
}
