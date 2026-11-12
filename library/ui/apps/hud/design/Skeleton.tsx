// Skeleton — a placeholder block with a band of light sweeping across it, shown where content
// is loading. Size and corners come from the caller's class; `tone` picks the surface.
import styles from './Skeleton.module.css'

export function Skeleton({ tone = 'default', className = '', style }: { tone?: 'default' | 'band'; className?: string; style?: React.CSSProperties }): React.JSX.Element {
  return <span className={`${styles.skeleton} ${styles[tone]} ${className}`.trim()} style={style} aria-hidden="true" />
}
