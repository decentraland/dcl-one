import type { ReactNode } from 'react'
import { Button } from '../shared/Button/Button'
import { Modal } from '../shared/Modal/Modal'
import { GeneralCheckCircle } from '../../../foundations/icons'
import {
  DownloadActions,
  StoreBadge,
  type DownloadActionsProps,
  type StoreBadgeProps
} from './DownloadActions'
import styles from './DownloadPage.module.css'

type DownloadPageProps = {
  /** The page heading, e.g. "Download Decentraland". */
  title: ReactNode
  /** Signed-in line above the title, e.g. "Alex is Ready!". */
  preTitle?: ReactNode
  /** Keep the first row empty on phones, where the compact Sites header (SitesNavbar `compact`) sits
      over the page; from desktop up it floats and needs no room. Spacing only. */
  reserveHeaderRow?: boolean
  /** "Already downloaded? Jump in", desktop only. */
  jumpIn?: { text: ReactNode; label: ReactNode; onClick: () => void }
  primary?: DownloadActionsProps['primary']
  epic?: DownloadActionsProps['epic']
  downloads?: ReactNode
  alternatives?: DownloadActionsProps['alternatives']
  /** Phones see one official store badge instead of installers. */
  store?: StoreBadgeProps
  /** The signed-in avatar preview. */
  preview?: ReactNode
  /** Shown when Jump In found no installed app. */
  launcherMissing?: {
    titleLines: [ReactNode, ReactNode]
    cta: ReactNode
    closeLabel: string
    onClose: () => void
  } | null
  /** Rendered after the page content, for the app footer. */
  afterContent?: ReactNode
}

/** The /download page body: installer options on wide screens, a store badge on narrow ones. With a
    store, both layouts render and the viewport width picks one, as source does. */
function DownloadPage({
  title,
  preTitle,
  reserveHeaderRow,
  jumpIn,
  primary,
  epic,
  downloads,
  alternatives,
  store,
  preview,
  launcherMissing,
  afterContent
}: DownloadPageProps) {
  return (
    <>
      <main className={preview ? `${styles.root} ${styles.withPreview}` : styles.root}>
        {reserveHeaderRow ? (
          // An empty, hidden spacer has no role or name, so tests find it by its test id.
          <div className={styles.headerRow} aria-hidden="true" data-testid="download-header-row" />
        ) : null}

        <div className={store ? `${styles.content} ${styles.wideOnly}` : styles.content}>
          {preTitle ? (
            <p className={styles.preTitle}>
              <GeneralCheckCircle size={32} aria-hidden="true" className={styles.check} />
              {preTitle}
            </p>
          ) : null}
          <h1 className={styles.title}>{title}</h1>

          <DownloadActions
            primary={primary}
            epic={epic}
            downloads={downloads}
            alternatives={alternatives}
          />
        </div>

        {store ? (
          <>
            <div className={`${styles.content} ${styles.contentTop} ${styles.narrowOnly}`}>
              <h1 className={styles.title}>{title}</h1>
            </div>
            <div className={`${styles.storeBar} ${styles.narrowOnly}`}>
              <StoreBadge {...store} />
            </div>
          </>
        ) : null}

        {jumpIn ? (
          <p className={store ? `${styles.jumpIn} ${styles.wideOnly}` : styles.jumpIn}>
            {jumpIn.text}{' '}
            <Button variant="text" onClick={jumpIn.onClick}>
              {jumpIn.label}
            </Button>
          </p>
        ) : null}

        {preview ? <div className={styles.preview}>{preview}</div> : null}
      </main>

      {launcherMissing ? (
        <Modal
          title={
            <>
              {launcherMissing.titleLines[0]}
              <br />
              {launcherMissing.titleLines[1]}
            </>
          }
          closeLabel={launcherMissing.closeLabel}
          onClose={launcherMissing.onClose}
          actions={<Button onClick={launcherMissing.onClose}>{launcherMissing.cta}</Button>}
        />
      ) : null}
      {afterContent}
    </>
  )
}

export { DownloadPage }
export type { DownloadPageProps }
