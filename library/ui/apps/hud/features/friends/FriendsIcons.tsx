// The friends panel's own glyphs, drawn for it.

/** The no-friends badge: a framed person with a heart, the shoulder's near side in orange. */
export function FriendsBadge({ size = 72 }: { size?: number }): React.JSX.Element {
  return (
    <svg viewBox="0 0 72 72" width={size} height={size} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
      <rect x="1.5" y="1.5" width="69" height="69" rx="15" />
      <circle cx="33" cy="21" r="7.5" />
      <path d="M19 56.5c-.5-10 4.5-18 13-19M19 56.5h15" />
      <path d="M24 53.5c.5-6 3.5-10.5 8.5-12l4 12Z" fill="var(--accent)" stroke="none" />
      <path transform="translate(33.4 34.6) scale(1.1 .96)" d="M12 21.3C5.6 16.4 1.5 12.4 1.5 7.6 1.5 4 4.2 1.5 7.4 1.5c1.9 0 3.6.9 4.6 2.4 1-1.5 2.7-2.4 4.6-2.4 3.2 0 5.9 2.5 5.9 6.1 0 4.8-4.1 8.8-10.5 13.7Z" fill="currentColor" stroke="none" />
    </svg>
  )
}

/** The profile menu's kebab as the hint shows it: three dots on a dark tab. */
export function KebabTab(): React.JSX.Element {
  return (
    <svg viewBox="0 0 10 18" width="10" height="18" aria-hidden="true">
      <rect width="10" height="18" rx="3" fill="rgba(255, 255, 255, 0.22)" />
      <circle cx="5" cy="4.5" r="1.5" fill="currentColor" />
      <circle cx="5" cy="9" r="1.5" fill="currentColor" />
      <circle cx="5" cy="13.5" r="1.5" fill="currentColor" />
    </svg>
  )
}
