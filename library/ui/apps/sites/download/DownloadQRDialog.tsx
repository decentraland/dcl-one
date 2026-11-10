import type { ReactNode } from 'react'
import { Modal } from '../shared/Modal/Modal'
import { GeneralVerified } from '../../../foundations/icons'
import qrLogo from '../assets/qr/dcl-logo-qr.svg'
import qrAndroid from '../assets/qr/qr-android.svg'
import qrIos from '../assets/qr/qr-ios.svg'
import styles from './DownloadQRDialog.module.css'

type DownloadQRDialogProps = {
  os: 'ios' | 'android'
  /** e.g. "Download Decentraland App for iOS". */
  title: ReactNode
  /** What the code opens, e.g. "QR code to download Decentraland for iOS". */
  qrLabel: string
  /** e.g. "Total Downloads: +400K". */
  downloads?: ReactNode
  closeLabel: string
  onClose: () => void
}

/** Desktop visitors pick a phone store by scanning its code (ui2 DownloadQRModal). */
function DownloadQRDialog({
  os,
  title,
  qrLabel,
  downloads,
  closeLabel,
  onClose
}: DownloadQRDialogProps) {
  return (
    <Modal title={title} closeLabel={closeLabel} onClose={onClose}>
      <div className={styles.body}>
        <div className={styles.code}>
          <img className={styles.qr} src={os === 'ios' ? qrIos : qrAndroid} alt={qrLabel} />
          <img className={styles.logo} src={qrLogo} alt="" />
        </div>
        {downloads ? (
          <p className={styles.downloads}>
            <GeneralVerified size={20} aria-hidden="true" />
            {downloads}
          </p>
        ) : null}
      </div>
    </Modal>
  )
}

export { DownloadQRDialog }
export type { DownloadQRDialogProps }
