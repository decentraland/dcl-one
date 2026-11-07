// from: shop@3a322cf9:app/src/components/EmptyState/EmptyState.tsx (the dark panel: art, title, body, one CTA)
// The Shop's empty-state panel, shared by the pages that say "nothing here yet" (My Favorites, My Items).
import { Link } from "react-router";

import styles from "./EmptyPanel.module.css";

export type EmptyCta = { label: string; to: string } | { label: string; onClick: () => void };

export function EmptyPanel({ icon, title, body, cta, testId }: { icon: string; title: string; body?: string; cta?: EmptyCta; testId?: string }) {
  return (
    <div className={styles.panel} data-testid={testId}>
      <img className={styles.img} src={icon} alt="" aria-hidden />
      <div className={styles.text}>
        <p className={styles.title}>{title}</p>
        {body ? <p className={styles.body}>{body}</p> : null}
      </div>
      {cta ? (
        "to" in cta ? (
          <Link className={styles.cta} to={cta.to}>
            {cta.label}
          </Link>
        ) : (
          <button type="button" className={styles.cta} onClick={cta.onClick}>
            {cta.label}
          </button>
        )
      ) : null}
    </div>
  );
}
