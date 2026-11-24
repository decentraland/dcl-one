// What the scene the player is standing in turns off: an icon in the minimap header, shown only
// while something is restricted. Hovering or focusing it shows the list, beside the icon.

import { useId } from 'react'
import { Info, Tooltip } from '../../design'
import styles from './SceneRestrictions.module.css'

export function SceneRestrictions({ restrictions }: { restrictions: string[] }): React.JSX.Element {
  const listId = useId()
  return (
    <Tooltip id={listId} title="This scene is restricting the use of the following features" label={restrictions.map((r) => `\u2022 ${r}`).join('\n')}>
      {/* focusable so the list is reachable without a pointer; the list is its description */}
      <span className={styles.icon} role="img" tabIndex={0} aria-label="Scene restrictions" aria-describedby={listId}>
        <Info size={16} />
      </span>
    </Tooltip>
  )
}
