import { useId, type ReactNode } from 'react'
import background from '../assets/come_hang_out_background.webp'
import {
  DownloadActions,
  StoreBadge,
  type DownloadActionsProps,
  type StoreBadgeProps
} from '../download/DownloadActions'
import styles from './ComeHangOut.module.css'

type ComeHangOutProps = {
  title: string
  /** Decorative background supplied by the owning route. */
  background?: ReactNode
  /** Desktop: the download actions. */
  actions?: DownloadActionsProps
  /** Phones: one store badge instead. */
  store?: StoreBadgeProps
}

/** The closing download call on the home page: the title and actions above the avatar group, as
    source. */
function ComeHangOut({ title, background: decoration, actions, store }: ComeHangOutProps) {
  const titleId = useId()
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      {decoration ? (
        <div className={styles.background} aria-hidden="true" inert>
          {decoration}
        </div>
      ) : null}
      <img className={styles.avatars} src={background} alt="" loading="lazy" />
      <div className={styles.content}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        {store ? (
          <StoreBadge {...store} appearance="source" />
        ) : actions ? (
          <DownloadActions {...actions} align="center" appearance="source" />
        ) : null}
      </div>
    </section>
  )
}

export { ComeHangOut }
export type { ComeHangOutProps }
