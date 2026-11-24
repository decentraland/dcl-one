// What the scene the player is standing in turns off: an icon in the minimap header, shown only
// while something is restricted. Hovering it fades in the list, beside the icon.

import { Info } from '../../design'
import styles from './SceneRestrictions.module.css'

export function SceneRestrictions({ restrictions }: { restrictions: string[] }): React.JSX.Element {
  return (
    <span className={styles.wrap}>
      <span className={styles.icon} role="img" aria-label={`Scene restrictions: ${restrictions.join(', ')}`}>
        <Info size={16} />
      </span>
      <span className={styles.panel} role="tooltip">
        <span className={styles.head}>This scene is restricting the use of the following features</span>
        <span className={styles.list}>
          {restrictions.map((restriction) => (
            <span key={restriction} className={styles.line}>
              {`• ${restriction}`}
            </span>
          ))}
        </span>
      </span>
    </span>
  )
}
