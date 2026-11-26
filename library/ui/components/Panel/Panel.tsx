import type { HTMLAttributes } from 'react'
import styles from './Panel.module.css'

export type PanelProps = HTMLAttributes<HTMLDivElement> & {
  /** A frosted, more translucent surface, for a panel over the world. */
  blur?: boolean
}

/** The dark surface of a HUD widget or menu. The caller sizes it and owns its role and content. */
export function Panel({ blur = false, className, ...rest }: PanelProps) {
  return (
    <div
      {...rest}
      className={[styles.root, blur && styles.blur, className].filter(Boolean).join(' ')}
      data-blur={blur || undefined}
    />
  )
}
