// The sign-in screens' BACK: a chevron in a faint square, then the word.
import { Button, MaskIcon } from '../../design'
import arrowLeft from '../../assets/startup/arrow-left.png'
import styles from './StartupBack.module.css'

export function StartupBack({ onClick, className }: { onClick: () => void; className?: string }): React.JSX.Element {
  return (
    <Button size="bare" variant="clear" className={`${styles.back} ${className ?? ''}`} onClick={onClick}>
      <span className={styles.square}>
        <MaskIcon src={arrowLeft} size={20} />
      </span>
      BACK
    </Button>
  )
}
