// The chat's own glyphs, drawn for it: the title bar's kebab, the heart beside the input and the
// options menu's bell, translate and trash.

interface IconProps {
  size?: number
}

const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' } as const

/** Three round dots in a column: the conversation options tab. */
export function KebabDots(): React.JSX.Element {
  return (
    <svg viewBox="0 0 6 20" width="6" height="20" aria-hidden="true" fill="currentColor">
      <circle cx="3" cy="3" r="2.5" />
      <circle cx="3" cy="10" r="2.5" />
      <circle cx="3" cy="17" r="2.5" />
    </svg>
  )
}

/** Wider than tall: about 19x17 drawn. */
export function HeartIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 22" width="22" height="19" preserveAspectRatio="none" aria-hidden="true">
      <path fill="currentColor" d="M12 21.3C5.6 16.4 1.5 12.4 1.5 7.6 1.5 4 4.2 1.5 7.4 1.5c1.9 0 3.6.9 4.6 2.4 1-1.5 2.7-2.4 4.6-2.4 3.2 0 5.9 2.5 5.9 6.1 0 4.8-4.1 8.8-10.5 13.7Z" />
    </svg>
  )
}

export function BellIcon({ size = 22 }: IconProps): React.JSX.Element {
  return (
    <svg viewBox="0 0 20 20" width={size} height={size} aria-hidden="true" {...stroke}>
      <path d="M4.5 14.2V9a5.5 5.5 0 0 1 11 0v5.2" />
      <path d="M2.6 14.4h14.8" />
      <path d="M8 17.2a2.2 2.2 0 0 0 4 0" />
    </svg>
  )
}

/** A han character and a Latin A with the two turning arrows between them. */
export function TranslateIcon({ size = 20 }: IconProps): React.JSX.Element {
  return (
    <svg viewBox="0 0 20 20" width={size} height={size} aria-hidden="true" {...stroke} strokeWidth={1.9}>
      <path d="M1.8 4h7.4M5.5 2.2V4M3 4c.4 2.6 2.4 4.8 5.4 5.8M8 4C7.6 6.6 5.4 9 2 10" />
      <path d="m10 18.4 3.6-9 3.6 9M11.3 15.6h4.6" />
      <path d="M12.2 2.6h1.6a3 3 0 0 1 3 3v.8M15.2 5.2l1.6 1.6 1.6-1.6" />
      <path d="M7.8 17.4H6.2a3 3 0 0 1-3-3v-.8M4.8 14.8l-1.6-1.6-1.6 1.6" />
    </svg>
  )
}

export function TrashOutlineIcon({ size = 20 }: IconProps): React.JSX.Element {
  return (
    <svg viewBox="0 0 20 20" width={size} height={size} aria-hidden="true" {...stroke}>
      <path d="M1.8 5.2h16.4M7 5V3.2a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1V5" />
      <path d="M3.4 5.4l.8 11a1.6 1.6 0 0 0 1.6 1.4h8.4a1.6 1.6 0 0 0 1.6-1.4l.8-11" />
      <path d="M7.2 8.6v5.8M10 8.6v5.8M12.8 8.6v5.8" />
    </svg>
  )
}
