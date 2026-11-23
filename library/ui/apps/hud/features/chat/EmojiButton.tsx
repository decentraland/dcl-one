// The emoji panel's toggle the chat input and the announcement composer share. It keeps the
// input's focus when pressed; `className` places it.
import styles from './EmojiButton.module.css'

export function EmojiButton({ open, onToggle, className, ref }: { open: boolean; onToggle: () => void; className?: string; ref?: React.Ref<HTMLButtonElement> }): React.JSX.Element {
  return (
    <button
      ref={ref}
      type="button"
      className={[styles.button, open ? styles.on : '', className ?? ''].filter(Boolean).join(' ')}
      aria-label="Emoji"
      aria-pressed={open}
      data-ui-sound="soft"
      onMouseDown={(e) => e.preventDefault()}
      onClick={onToggle}
    />
  )
}
