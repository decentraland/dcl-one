// What the scene the player is standing in turns off: an icon in the minimap header, shown only
// while something is restricted, with the list on hover.

import { Info, Tooltip } from '../../design'
import styles from './SceneRestrictions.module.css'

export function SceneRestrictions({ restrictions }: { restrictions: string[] }): React.JSX.Element {
  return (
    <Tooltip label={restrictions.map((r) => `\u2022 ${r}`).join('\n')} side="bottom" multiline>
      <span className={styles.icon} role="img" aria-label={`Scene restrictions: ${restrictions.join(', ')}`}>
        <Info size={16} />
      </span>
    </Tooltip>
  )
}
