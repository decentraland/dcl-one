import { useEffect } from 'react'
import {
  CheckCircleRoundedIcon as SuccessIcon,
  CloseIcon,
  ErrorRoundedIcon as ErrorIcon,
  InfoRoundedIcon as InfoIcon,
  WarningRoundedIcon as WarnIcon
} from '~/components/Icons'
import { useNotifications, type Toast, type ToastType } from '~/lib/notifications'
import { useTranslation } from '~/intl'
import styles from './Toasts.module.css'

const ICONS: Record<NonNullable<ToastType>, typeof SuccessIcon> = {
  success: SuccessIcon,
  error: ErrorIcon,
  warn: WarnIcon,
  info: InfoIcon
}

function ToastItem({ toast }: { toast: Toast }) {
  const dismissToast = useNotifications(state => state.dismissToast)
  const { t } = useTranslation()
  const Icon = toast.type && ICONS[toast.type]

  useEffect(() => {
    const timeout = window.setTimeout(() => dismissToast(toast.id), toast.durationMs)
    return () => window.clearTimeout(timeout)
  }, [toast.id, toast.durationMs, dismissToast])

  return (
    <div
      className={styles.toastCard}
      role={toast.type === 'error' ? 'alert' : 'status'}
      data-testid="toast"
      data-type={toast.type ?? undefined}
    >
      {Icon && (
        <span className={styles.typeIcon} data-type={toast.type} data-testid="toast-icon">
          <Icon fontSize="small" />
        </span>
      )}
      {toast.message}
      <button
        className={styles.dismissButton}
        type="button"
        aria-label={t('modal.close')}
        onClick={() => dismissToast(toast.id)}
      >
        <CloseIcon fontSize="small" />
      </button>
    </div>
  )
}

/** Global toast outlet, fed by the lib/notifications store. Mounted once in App. */
export function Toasts() {
  const toasts = useNotifications(state => state.toasts)
  if (toasts.length === 0) return null
  return (
    <div className={styles.stack} data-testid="toasts">
      {toasts.map(toast => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  )
}
