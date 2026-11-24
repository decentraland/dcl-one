import { useState, type ReactNode } from 'react'
import type { BodyShape } from '@dcl/schemas'
import { CloseIcon, DesktopWindowsOutlinedIcon as DesktopIcon } from '../../Icons'
import { ItemFileThumbnail } from '../../ItemThumbnail'
import { useTranslation } from '@/data/lib/creators/intl'
import { hasRepresentationFor } from '@/data/lib/creators/itemEditor'
import type { Item } from '@/data/lib/creators/items'
import styles from '../ItemEditorPage.module.css'

type Props = {
  items: Item[]
  selectedId: string | null
  dressedIds: string[]
  bodyShape: BodyShape
  /** Tapping a thumbnail puts the item on the avatar, or takes it off when it is already on. */
  onTap: (item: Item) => void
  /** The avatar preview with its overlay controls. */
  children: ReactNode
  /** The bar above the preview: back link, collection name and status. */
  header?: ReactNode
  /** Review mode: curators only look on phones, so the editing hint is left out. */
  readOnly?: boolean
}

/**
 * Phone layout: the preview fills the screen and a thumbnail strip is the only item control, so each
 * thumbnail is a toggle — there is no dress button as on desktop.
 */
export function MobilePreview({ items, selectedId, dressedIds, bodyShape, onTap, children, header, readOnly }: Props) {
  const { t } = useTranslation()
  const [hintDismissed, setHintDismissed] = useState(false)
  return (
    <div className={styles.mobileWorkspace} data-testid="item-editor-mobile">
      {header}
      <div className={styles.mobilePreviewArea}>{children}</div>
      {!hintDismissed && !readOnly && (
        <div className={styles.mobileHint} data-testid="mobile-hint">
          <DesktopIcon fontSize="small" />
          <span>{t('item_editor.mobile_hint')}</span>
          <button
            type="button"
            aria-label={t('modal.close')}
            data-testid="mobile-hint-dismiss"
            onClick={() => setHintDismissed(true)}
          >
            <CloseIcon fontSize="small" />
          </button>
        </div>
      )}
      <div className={styles.strip} role="group" aria-label={t('item_editor.sidebar.items')} data-testid="mobile-strip">
        {items.map(item => {
          const available = hasRepresentationFor(item, bodyShape)
          const dressed = dressedIds.includes(item.id)
          const thumbnail = item.contents[item.thumbnail]
          return (
            <button
              className={styles.stripItem}
              key={item.id}
              type="button"
              aria-pressed={dressed}
              aria-current={item.id === selectedId || undefined}
              aria-label={t(dressed ? 'item_editor.sidebar.undress' : 'item_editor.sidebar.dress', {
                name: item.name
              })}
              title={item.name}
              data-selected={item.id === selectedId || undefined}
              data-dressed={dressed || undefined}
              data-unavailable={!available || undefined}
              data-testid={`mobile-strip-item-${item.id}`}
              onClick={() => onTap(item)}
            >
              <ItemFileThumbnail item={item} hash={thumbnail} rarity={item.rarity} />
            </button>
          )
        })}
      </div>
    </div>
  )
}
