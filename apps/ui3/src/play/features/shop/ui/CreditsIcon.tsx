// CreditsIcon — the New Shop's credit mark (its "credits-icon" component): the octagon, its rim and the C,
// layered as the design draws them, in a square box of any size (the mark fills 79% by 83% of it).

import c from '../../../assets/shop/credits-icon/c.svg'
import v0 from '../../../assets/shop/credits-icon/v0.svg'
import v1 from '../../../assets/shop/credits-icon/v1.svg'
import v2 from '../../../assets/shop/credits-icon/v2.svg'
import v3 from '../../../assets/shop/credits-icon/v3.svg'
import v4 from '../../../assets/shop/credits-icon/v4.svg'
import v5 from '../../../assets/shop/credits-icon/v5.svg'
import v6 from '../../../assets/shop/credits-icon/v6.svg'
import v7 from '../../../assets/shop/credits-icon/v7.svg'
import v8 from '../../../assets/shop/credits-icon/v8.svg'
import styles from './CreditsIcon.module.css'

const LAYERS: [string, string][] = [
  [v0, styles.v0],
  [v1, styles.v1],
  [v2, styles.v2],
  [v3, styles.v3],
  [v4, styles.v4],
  [v5, styles.v5],
  [v6, styles.v6],
  [v7, styles.v7],
  [v8, styles.v8],
  [c, styles.c]
]

export function CreditsIcon({ size, className }: { size: number; className?: string }): React.JSX.Element {
  return (
    <span className={`${styles.icon} ${className ?? ''}`.trim()} style={{ width: size, height: size }} aria-hidden="true">
      {LAYERS.map(([src, cls]) => (
        <span key={cls} className={`${styles.layer} ${cls}`}>
          <img src={src} alt="" />
        </span>
      ))}
    </span>
  )
}
