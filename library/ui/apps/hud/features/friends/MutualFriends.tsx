import { Avatar } from '../../design'
import { knownUserColor } from '../session/profileStore'
import type { MutualFriend } from '../session/useEngineSession'
import styles from './MutualFriends.module.css'

export function MutualFriends({ list, size = 24, className }: { list: MutualFriend[] | undefined; size?: number; className?: string }): React.JSX.Element | null {
  if (!list || list.length === 0) return null
  return (
    <div className={[styles.mutuals, className].filter(Boolean).join(' ')}>
      {list.slice(0, 3).map((m) => (
        <Avatar key={m.address} src={m.picture} name={m.name} color={knownUserColor(m.address, m.name)} size={size} className={styles.face} />
      ))}
      <span>{list.length} Mutual</span>
    </div>
  )
}
