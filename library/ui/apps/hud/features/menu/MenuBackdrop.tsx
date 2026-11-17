// MenuBackdrop — the one page background of every full-screen menu page:
// purple, lighter down the left side and deeper down the right, and the page's watermark glyph
// where it has one. Pages that paint edge to edge mount it themselves; the shell mounts it for the rest.

import styles from './MenuBackdrop.module.css'

function Gear({ className }: { className: string }): React.JSX.Element {
  return (
    <svg className={className} viewBox="-100 -100 200 200" aria-hidden="true">
      <g fill="currentColor">
        {[0, 45, 90, 135, 180, 225, 270, 315].map((a) => (
          <rect key={a} x="-21" y="-100" width="42" height="46" rx="11" transform={`rotate(${a})`} />
        ))}
        <path fillRule="evenodd" d="M0-80a80 80 0 1 0 0 160a80 80 0 1 0 0-160zM0-36a36 36 0 1 1 0 72a36 36 0 1 1 0-72z" />
      </g>
    </svg>
  )
}

export function MenuBackdrop({ watermark, className }: { watermark?: 'settings'; className?: string }): React.JSX.Element {
  return (
    <div className={`${styles.backdrop} ${className ?? ''}`.trim()} aria-hidden="true">
      {watermark === 'settings' && (
        <>
          <Gear className={`${styles.mark} ${styles.gearSmall}`} />
          <Gear className={`${styles.mark} ${styles.gearBig}`} />
        </>
      )}
    </div>
  )
}
