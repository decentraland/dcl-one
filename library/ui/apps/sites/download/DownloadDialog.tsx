import { Modal } from '../shared/Modal/Modal'
import {
  DownloadActions,
  StoreBadge,
  type DownloadActionsProps,
  type StoreBadgeProps
} from './DownloadActions'
import styles from './DownloadDialog.module.css'

type DownloadDialogProps = {
  /** e.g. "Download Decentraland to Jump In". */
  title: string
  /** Desktop: the download actions. */
  actions?: DownloadActionsProps
  /** Phones: one store badge instead. */
  store?: StoreBadgeProps
  closeLabel: string
  onClose: () => void
}

/** "Install first" (ui2 DownloadModal): shown when Hang Out finds no installed app. */
function DownloadDialog({ title, actions, store, closeLabel, onClose }: DownloadDialogProps) {
  return (
    <Modal title={title} closeLabel={closeLabel} onClose={onClose}>
      <div className={styles.body}>
        {store ? <StoreBadge {...store} /> : actions ? <DownloadActions {...actions} /> : null}
      </div>
    </Modal>
  )
}

export { DownloadDialog }
export type { DownloadDialogProps }
