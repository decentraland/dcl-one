import type { ReactNode } from "react";
import ChromeLink from "./ChromeLink";
import { flatIcon } from "../../atoms/MaskIcon";
import type { ChromeNavigate } from "./chrome-nav";
import "../../components/sidenavitem.css";

type Item<Id extends string> = { id: Id; label: ReactNode; href?: string; icon?: ReactNode; className?: string };
export default function SiteSectionNavigation<Id extends string>({
  title, label = typeof title === "string" ? title + " sections" : "Section navigation", items, active, onSelect, onNavigate, closeMenu, actions,
}: {
  title: ReactNode;
  label?: string;
  items: readonly Item<Id>[];
  active?: NoInfer<Id> | null;
  onSelect?: (id: NoInfer<Id>) => void;
  onNavigate?: ChromeNavigate;
  closeMenu?: () => void;
  actions?: ReactNode;
}) {
  return (
    <nav className="site-sections" aria-label={label}>
      {title != null ? <div className="site-sections__title">{title}</div> : null}
      {actions}
      {items.map(item => {
        const props = {
          className: "side-nav-item" + (item.id === active ? " is-active" : "") + (item.className ? " " + item.className : ""),
          "aria-current": item.id === active ? "page" as const : undefined,
          onClick: () => { if (!item.href) onSelect?.(item.id); closeMenu?.(); },
        };
        const content = <>{item.icon ? flatIcon(item.icon) : null}{item.label}</>;
        return item.href ? (
          <ChromeLink key={item.id} href={item.href} onNavigate={onNavigate} {...props}>{content}</ChromeLink>
        ) : <button key={item.id} type="button" {...props}>{content}</button>;
      })}
    </nav>
  );
}
