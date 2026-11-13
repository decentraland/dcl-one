// Settings → Lobby: the lobby stage's backdrop. A large preview of the hovered look (else the chosen
// one) over a thumbnail per look; picking one changes the stage at once and is kept on this device.

import { useState } from 'react'
import checkmark from '../../assets/lobby/checkmark-green.png'
import { LOBBY_STAGE_PRESETS, lobbyStagePreset } from '@explorer-core/engine/lobbyStage'
import { STAGE_BACKDROPS } from '../lobby/stageBackdrops'
import styles from './LobbyStageTab.module.css'

export function LobbyStageTab({ preset, onPick }: { preset: string; onPick: (id: string) => void }): React.JSX.Element {
  const chosen = lobbyStagePreset(preset).id
  const [hovered, setHovered] = useState<string | null>(null)
  const shown = hovered ?? chosen
  return (
    <div className={styles.tab}>
      <h2 className={styles.title}>Background</h2>
      <img className={styles.preview} src={STAGE_BACKDROPS[shown]} alt={lobbyStagePreset(shown).name} />
      <div className={styles.options} role="radiogroup" aria-label="Lobby background">
        {LOBBY_STAGE_PRESETS.map((p) => (
          <button
            key={p.id}
            type="button"
            role="radio"
            aria-checked={p.id === chosen}
            title={p.name}
            className={`${styles.option} ${p.id === chosen ? styles.selected : ''}`.trim()}
            onClick={() => onPick(p.id)}
            onMouseEnter={() => setHovered(p.id)}
            onMouseLeave={() => setHovered(null)}
            onFocus={() => setHovered(p.id)}
            onBlur={() => setHovered(null)}
          >
            <img className={styles.thumb} src={STAGE_BACKDROPS[p.id]} alt={p.name} />
            {p.id === chosen && <img className={styles.check} src={checkmark} alt="" width={20} height={20} />}
          </button>
        ))}
      </div>
    </div>
  )
}
