// The sign-in screens' BACK: a chevron in a faint square, then the word.
import arrowLeft from '../../assets/startup/arrow-left.png'
import styles from './StartupBack.module.css'
import { Button, MaskIcon } from '@dcl/one-ui'

export function StartupBack({ onClick, className }: { onClick: () => void; className?: string }): React.JSX.Element {
  return (
    <Button appearance="hud" size="bare" variant="clear" className={`${styles.back} ${className ?? ''}`} onClick={onClick}>
      <span className={styles.square}>
        <MaskIcon src={arrowLeft} size={20} />
      </span>
      BACK
    </Button>
  )
}
