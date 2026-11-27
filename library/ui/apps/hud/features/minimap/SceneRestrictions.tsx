// What restricts the player in the scene they stand in, in the minimap header: a mark for a scene
// or world ban, and an icon for what the scene turns off, shown only while something is
// restricted, each with its explanation on hover.

import { BanIcon, Info } from '../../design'
import type { SceneBanScope } from '@explorer-core/engine/generated'
import { SCENE_BAN_LABEL, SCENE_BAN_NOTICE } from './sceneBan'
import styles from './SceneRestrictions.module.css'
import { Tooltip } from '@dcl/one-ui'

export function SceneRestrictions({ restrictions, ban = null }: { restrictions: string[]; ban?: SceneBanScope | null }): React.JSX.Element {
  return (
    <>
      {ban != null && (
        <Tooltip appearance="hud" label={SCENE_BAN_NOTICE[ban]} side="bottom" multiline>
          <span className={`${styles.icon} ${styles.ban}`} role="img" aria-label={SCENE_BAN_LABEL[ban]}>
            <BanIcon size={16} />
          </span>
        </Tooltip>
      )}
      {restrictions.length > 0 && (
        <Tooltip appearance="hud" label={restrictions.map((r) => `\u2022 ${r}`).join('\n')} side="bottom" multiline>
          <span className={styles.icon} role="img" aria-label={`Scene restrictions: ${restrictions.join(', ')}`}>
            <Info size={16} />
          </span>
        </Tooltip>
      )}
    </>
  )
}
