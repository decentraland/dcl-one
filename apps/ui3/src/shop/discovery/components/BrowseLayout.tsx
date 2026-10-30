// from: shop@94584a5c:app/src/pages/Assets.styles.ts, app/src/styles/browseLayout.styles.ts (the sidebar + main
//       grid and its mobile bottom-sheet drawer, closed on Escape or the scrim, body scroll locked while open)

import { useEffect, type ReactNode } from "react";

import { t } from "../intl";
import Icon from "./Icon";
import "./browselayout.css";

export default function BrowseLayout({
  sidebar,
  sidebarTop,
  children,
  open,
  onClose,
  testId = "browse",
}: {
  sidebar: ReactNode;
  sidebarTop?: ReactNode;
  children: ReactNode;
  open: boolean;
  onClose: () => void;
  testId?: string;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [open, onClose]);

  return (
    <div className="shop-browse" data-testid={testId}>
      {open ? <div className="shop-browse__scrim" onClick={onClose} aria-hidden /> : null}
      <div className="shop-browse__col">
        {sidebarTop}
        <aside className="shop-browse__sidebar" data-open={open || undefined} data-testid="browse-sidebar" aria-label={t("assets.filters")}>
          <div className="shop-browse__drawerhead">
            <h2>{t("assets.filters")}</h2>
            <button type="button" onClick={onClose} aria-label={t("assets.closeFilters")}>
              <Icon name="close" size={18} />
            </button>
          </div>
          <div className="shop-browse__scroll">{sidebar}</div>
          <div className="shop-browse__drawerfoot">
            <button type="button" onClick={onClose}>
              {t("assets.showItems")}
            </button>
          </div>
        </aside>
      </div>
      <div className="shop-browse__main">{children}</div>
    </div>
  );
}
