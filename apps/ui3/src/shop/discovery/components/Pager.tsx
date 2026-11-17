// from: shop@3a322cf9:app/src/pages/MyStore.tsx (Pager) and MyStore.styles.ts (Pager, PageBtn, PageNum, PageGap)
// The Shop's one pager: arrows with a hairline, page numbers as soft tiles, the current page the one solid
// control, an ellipsis for each skipped run. Always the same number of slots (pageWindow), so stepping
// through pages never moves the arrows.

import { pageWindow } from "../lib/pageWindow";
import { t } from "@shop/discovery/intl";
import Icon from "./Icon";
import styles from "./Pager.module.css";

export default function Pager({ page, pages, label, onPage, testId }: { page: number; pages: number; label: string; onPage: (page: number) => void; testId?: string }) {
  if (pages <= 1) return null;
  return (
    <nav className={styles.pager} aria-label={label} data-testid={testId}>
      <button type="button" className={styles.arrow} onClick={() => onPage(Math.max(0, page - 1))} disabled={page === 0} aria-label={t("overview.previous")}>
        <Icon name="chevron-down" size={16} aria-hidden className={styles.prev} />
      </button>
      {pageWindow(page, pages).map((n, i) =>
        n === "gap" ? (
          <span key={`gap-${i}`} className={styles.gap}>
            …
          </span>
        ) : (
          <button key={n} type="button" className={styles.num} aria-current={n === page ? "page" : undefined} onClick={() => onPage(n)}>
            {(n + 1).toLocaleString()}
          </button>
        ),
      )}
      <button type="button" className={styles.arrow} onClick={() => onPage(Math.min(pages - 1, page + 1))} disabled={page >= pages - 1} aria-label={t("overview.next")}>
        <Icon name="chevron-down" size={16} aria-hidden className={styles.next} />
      </button>
    </nav>
  );
}
