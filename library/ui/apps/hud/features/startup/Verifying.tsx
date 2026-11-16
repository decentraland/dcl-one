// "Verifying your account...": the sign-in and the account are being loaded. Cancel goes back to the
// first screen (the stored sign-in kept); it gives up on its own after a while (useEngineSession).
import { Button, Spinner } from '../../design'
import styles from './Verifying.module.css'

export function Verifying({ onCancel }: { onCancel?: () => void }): React.JSX.Element {
  return (
    <div className={styles.frame}>
      <div className={styles.spinner}>
        <Spinner size={70} variant="chase" />
      </div>
      <p className={styles.text}>Verifying your account...</p>
      {onCancel != null && (
        <Button size="bare" variant="clear" className={styles.cancel} onClick={onCancel}>
          Cancel
        </Button>
      )}
    </div>
  )
}
