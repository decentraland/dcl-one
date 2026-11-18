import { useState } from 'react'
import { MARKER_ART } from './mapArt'

export function QuestPinIcon({ icon, className, frameClassName }: { icon: string | null; className: string; frameClassName: string }): React.JSX.Element {
  const [broken, setBroken] = useState<string | null>(null)
  if (icon == null || icon === broken) return <img className={className} src={MARKER_ART.quest} alt="" draggable={false} />
  return (
    <span className={`${className} ${frameClassName}`}>
      <img src={icon} alt="" draggable={false} onError={() => setBroken(icon)} />
    </span>
  )
}
