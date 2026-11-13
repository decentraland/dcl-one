// The share menu of a place or an event: post on X, or copy its link.

import { useEffect, useRef } from 'react'
import type { ShareLinks } from '@explorer-core/features/map/shareLinks'
import { ContextMenu, showToast } from '../../../design'
import { MaskIcon } from '../../../design/MaskIcon'
import { PANEL_ART } from '../mapArt'
import styles from './MapPanel.module.css'

export function ShareMenu({ links, onClose }: { links: ShareLinks; onClose: () => void }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    ref.current?.querySelector('button')?.focus()
    const onDown = (e: PointerEvent): void => {
      if (ref.current != null && !ref.current.contains(e.target as Node)) onClose()
    }
    document.addEventListener('pointerdown', onDown, true)
    return () => document.removeEventListener('pointerdown', onDown, true)
  }, [onClose])
  return (
    <div
      ref={ref}
      className={styles.shareMenu}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation()
          onClose()
        }
      }}
    >
      <ContextMenu
        items={[
          {
            label: 'Share on X',
            icon: <MaskIcon src={PANEL_ART.twitter} size={18} />,
            onClick: () => {
              window.open(links.x, '_blank', 'noopener')
              onClose()
            }
          },
          {
            label: 'Copy Link',
            icon: <MaskIcon src={PANEL_ART.link} size={18} />,
            onClick: () => {
              void navigator.clipboard?.writeText(links.copy)
              showToast('Link copied!', { duration: 1000 })
              onClose()
            }
          }
        ]}
      />
    </div>
  )
}
