import { useState } from 'react'
import { MARKER_ART } from './mapArt'
import styles from './QuestPinIcon.module.css'

export function QuestPinIcon({ icon, className }: { icon: string | null; className: string }): React.JSX.Element {
  const [broken, setBroken] = useState<string | null>(null)
  if (icon == null || icon === broken) return <img className={className} src={MARKER_ART.quest} alt="" draggable={false} />
  return (
    <span className={`${className} ${styles.frame}`}>
      <img src={icon} alt="" draggable={false} onError={() => setBroken(icon)} />
    </span>
  )
}
