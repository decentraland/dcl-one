import {
  DownloadActions,
  StoreBadge,
  type DownloadActionsProps,
  type StoreBadgeProps
} from './DownloadActions'
import styles from './DownloadPage.module.css'

type DownloadPageProps = {
  title: string
  actions: DownloadActionsProps
  store: StoreBadgeProps
}

function DownloadPage({ title, actions, store }: DownloadPageProps) {
  return (
    <main className={styles.root}>
      <section className={styles.content} aria-labelledby="download-title">
        <h1 className={styles.title} id="download-title">
          {title}
        </h1>
        <div className={styles.desktopActions}>
          <DownloadActions {...actions} appearance="source" />
        </div>
        <div className={styles.mobileStore}>
          <StoreBadge {...store} appearance="source" />
        </div>
      </section>
    </main>
  )
}

export { DownloadPage }
export type { DownloadPageProps }
