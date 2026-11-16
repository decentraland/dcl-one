// The map's search bar. Enter searches; while a place, an event or a category is shown the
// field is read-only and carries its name.

import type { CategoryKey, NavmapSnapshot } from '@explorer-core/features/map/navmap'
import { MaskIcon } from '../../../design/MaskIcon'
import { isCancelKey } from '../../../lib/bindingLabels'
import { CAT_ICONS, PANEL_ART } from '../mapArt'
import styles from './MapPanel.module.css'

const MAX_LENGTH = 40

export function SearchBar({
  input,
  onType,
  onSubmit,
  onClear,
  onBack
}: {
  input: NavmapSnapshot['input']
  onType: (text: string) => void
  onSubmit: () => void
  onClear: () => void
  onBack: () => void
}): React.JSX.Element {
  const category: CategoryKey | null = input.back ? null : input.category
  return (
    <div className={styles.searchBar}>
      {input.back ? (
        <button type="button" className={styles.searchBack} aria-label="Back" onClick={onBack}>
          <MaskIcon src={PANEL_ART.arrowLeft} size={14} />
        </button>
      ) : category != null ? (
        <img className={styles.searchCategory} src={CAT_ICONS[category]} alt="" />
      ) : (
        <MaskIcon className={styles.searchIcon} src={PANEL_ART.search} size={21} />
      )}
      <input
        className={styles.searchInput}
        type="text"
        aria-label="Search"
        placeholder="Search"
        maxLength={MAX_LENGTH}
        value={input.text}
        readOnly={!input.interactable}
        onFocus={(e) => e.currentTarget.select()}
        onChange={(e) => onType(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onSubmit()
          else if (isCancelKey(e) && input.text !== '') {
            // clearing takes this press; the next one is the page's
            e.stopPropagation()
            onClear()
          }
        }}
      />
      {input.clear && (
        <button type="button" className={styles.searchClear} aria-label="Clear search" onClick={onClear}>
          <MaskIcon src={PANEL_ART.closeThick} size={10} />
        </button>
      )}
    </div>
  )
}
