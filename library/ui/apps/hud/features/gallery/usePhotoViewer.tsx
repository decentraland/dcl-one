// Shows the photo viewer over the whole screen, whichever page, panel or popup asked for it: the
// viewer is a popup of its own, so it stacks by open order with the passport and the dialogs.

import { useEffect, useLayoutEffect, useState, useSyncExternalStore } from 'react'
import { openPopup, subscribePopups, topPopupOrder } from '../../design'
import { PhotoDetail } from './PhotoDetail'
import styles from './PhotoDetail.module.css'

export type PhotoViewerProps = Omit<React.ComponentProps<typeof PhotoDetail>, 'covered'>

/** A popup renders once, so the viewer reads its opener's current props from here. */
function photoSource(): {
  get: () => PhotoViewerProps | null
  set: (props: PhotoViewerProps | null) => void
  subscribe: (listener: () => void) => () => void
} {
  let props: PhotoViewerProps | null = null
  const listeners = new Set<() => void>()
  return {
    get: () => props,
    set: (next) => {
      props = next
      listeners.forEach((l) => l())
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    }
  }
}

function LivePhoto({ source }: { source: ReturnType<typeof photoSource> }): React.JSX.Element | null {
  const props = useSyncExternalStore(source.subscribe, source.get)
  // It mounts as the top popup; a later one (a passport, a confirm) covers it.
  const [own] = useState(topPopupOrder)
  const top = useSyncExternalStore(subscribePopups, topPopupOrder)
  return props != null ? <PhotoDetail {...props} covered={top !== own} /> : null
}

/** The viewer is open while `props` is set; `props.onClose` runs when the player closes it. */
export function usePhotoViewer(props: PhotoViewerProps | null): void {
  const [source] = useState(photoSource)
  useLayoutEffect(() => source.set(props))
  const open = props != null
  useEffect(() => {
    if (!open) return
    let closedByOpener = false
    const close = openPopup(() => <LivePhoto source={source} />, {
      className: styles.photoLayer,
      backdropClickCloses: false,
      onClose: () => {
        if (!closedByOpener) source.get()?.onClose()
      }
    })
    return () => {
      closedByOpener = true
      close()
    }
  }, [open, source])
}
