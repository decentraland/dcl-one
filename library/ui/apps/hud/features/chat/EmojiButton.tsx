// The emoji panel's toggle the chat input and the announcement composer share. It keeps the
// input's focus when pressed; `className` places it.
import styles from './EmojiButton.module.css'

export function EmojiButton({ open, onToggle, className }: { open: boolean; onToggle: () => void; className?: string }): React.JSX.Element {
  return (
    <button
      type="button"
      className={[styles.button, open ? styles.on : '', className ?? ''].filter(Boolean).join(' ')}
      aria-label="Emoji"
      aria-pressed={open}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onToggle}
    />
  )
}
