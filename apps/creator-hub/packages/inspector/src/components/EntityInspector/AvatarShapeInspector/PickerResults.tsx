import type { ReactNode } from 'react';

import styles from './PickerResults.module.css';

export type PickerResult = {
  id: string;
  name: string;
  thumbnail?: string;
  /** Shown under the name: a rarity, a file name, what choosing it will do. */
  meta?: string;
  /** Drawn when there is no thumbnail. */
  icon?: ReactNode;
  selected?: boolean;
};

type Props<T extends PickerResult> = {
  title: string;
  items: readonly T[];
  loading?: boolean;
  error?: string;
  empty: string;
  onSelect: (item: T) => void;
};

/** One titled group of choices in a picker dialog: thumbnails in a grid, or why there are none. */
export function PickerResults<T extends PickerResult>({
  title,
  items,
  loading,
  error,
  empty,
  onSelect,
}: Props<T>) {
  return (
    <section
      className={styles.section}
      aria-label={title}
      aria-busy={loading}
    >
      <h3 className={styles.title}>{title}</h3>
      {loading && !items.length ? (
        <p className={styles.note}>Loading…</p>
      ) : error ? (
        <p className={styles.error}>Could not load these: {error}</p>
      ) : !items.length ? (
        <p className={styles.note}>{empty}</p>
      ) : (
        <ul className={styles.list}>
          {items.map(item => (
            <li key={item.id}>
              <button
                type="button"
                className={styles.item}
                aria-pressed={item.selected}
                onClick={() => onSelect(item)}
                title={item.id}
              >
                {item.thumbnail ? (
                  <img
                    className={styles.thumb}
                    src={item.thumbnail}
                    alt=""
                    loading="lazy"
                  />
                ) : (
                  <span className={styles.thumb}>{item.icon}</span>
                )}
                <span className={styles.name}>{item.name}</span>
                {item.meta && <span className={styles.meta}>{item.meta}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
