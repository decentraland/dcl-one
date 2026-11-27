// The share menu of a place or an event: post on X, or copy its link. Opened from the keyboard its first item
// takes the focus; opened with the pointer, none. While open it is a cancel layer, so the engine's Cancel closes it
// wherever the focus is.

import { useCallback, useEffect, useRef, useState } from 'react'
import type { ShareLinks } from '@explorer-core/features/map/shareLinks'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { ContextMenu, showToast } from '../../../design'
import { PANEL_ART } from '../mapArt'
import styles from './MapPanel.module.css'
import { MaskIcon } from '@dcl/one-ui'
import { pressedWith, releasePointerFocus, type PressedWith } from '../../../lib/pressedWith'

/** A Share button's menu: how the button opened it, or null while it is closed. A pointer press leaves the button
 *  unfocused, as it leaves the menu. */
export function useShareMenu(): { openedWith: PressedWith | null; press: (event: React.MouseEvent<HTMLElement>) => void; close: () => void } {
  const [openedWith, setOpenedWith] = useState<PressedWith | null>(null)
  const press = useCallback((event: React.MouseEvent<HTMLElement>): void => {
    releasePointerFocus(event)
    setOpenedWith(pressedWith(event))
  }, [])
  const close = useCallback((): void => setOpenedWith(null), [])
  return { openedWith, press, close }
}

export function ShareMenu({ links, openedWith, onClose }: { links: ShareLinks; openedWith: PressedWith; onClose: () => void }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  // the latest `onClose`, so a parent drawing again does not take the focus back to the first item
  const close = useRef(onClose)
  close.current = onClose
  const fromKeyboard = useRef(openedWith === 'keyboard')
  useEffect(() => {
    if (fromKeyboard.current) ref.current?.querySelector('button')?.focus()
    const onDown = (e: PointerEvent): void => {
      if (ref.current != null && !ref.current.contains(e.target as Node)) close.current()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(() => close.current())
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [])
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
