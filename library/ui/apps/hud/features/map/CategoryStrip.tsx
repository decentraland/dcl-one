// The category chips along the top of the map. One may be on at a time; turning it on searches
// the category, turning it off clears the search.

import { CATEGORIES, type CategoryKey } from '@explorer-core/features/map/navmap'
import { CAT_ICONS } from './mapArt'
import styles from './MapChrome.module.css'

export function CategoryStrip({ selected, onSelect }: { selected: CategoryKey | null; onSelect: (key: CategoryKey | null) => void }): React.JSX.Element {
  return (
    <div className={styles.categories}>
      <div className={styles.categoryRow} role="group" aria-label="Categories">
        {CATEGORIES.map((c) => {
          const on = c.key === selected
          return (
            <button
              key={c.key}
              type="button"
              className={`${styles.category} ${on ? styles.categoryOn : ''}`.trim()}
              aria-pressed={on}
              onClick={() => onSelect(on ? null : c.key)}
            >
              <img className={styles.categoryIcon} src={CAT_ICONS[c.key]} alt="" />
              <span className={styles.categoryLabel}>{c.name}</span>
            </button>
          )
        })}
      </div>
    </div>
  )
}
