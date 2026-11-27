// The chat's own glyphs, drawn for it: the title bar's kebab and the heart beside the input.

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
