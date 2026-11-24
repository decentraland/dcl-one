import type { CSSProperties, SVGProps } from 'react'
import { cx } from '@/data/lib/creators/cx'
import styles from './SaleIcon.module.css'

// The Shop's glyphs for this modal, inlined with the same paths so both draw identically. Each fills or strokes with
// currentColor; `data-sale-icon` is the hook the modal's styles size and tint them by.
const GLYPHS = {
  alert: (
    <path
      fill="currentColor"
      d="M12 2 1 21h22L12 2zm0 6a1 1 0 0 1 1 1v5a1 1 0 1 1-2 0V9a1 1 0 0 1 1-1zm0 8.25a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5z"
    />
  ),
  'arrow-drop-down': <path d="M7 9.5L12 14.5L17 9.5H7Z" fill="currentColor" />,
  calendar: (
    <>
      <rect x="3.5" y="5" width="17" height="15.5" rx="2.5" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M3.5 10H20.5M8 3V7M16 3V7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </>
  ),
  check: <path d="M9.55 17.6 4.4 12.45l1.5-1.5 3.65 3.65 8.65-8.65 1.5 1.5z" fill="currentColor" />,
  'chevron-down': (
    <path
      transform="scale(1.2)"
      d="M13.825 6.9125L10 10.7292L6.175 6.9125L5 8.0875L10 13.0875L15 8.0875L13.825 6.9125Z"
      fill="currentColor"
    />
  ),
  'chevron-right': (
    <path
      transform="scale(1.0909)"
      d="M8.89633 5.50005L7.60382 6.79256L11.8022 11.0001L7.60382 15.2076L8.89633 16.5001L14.3964 11.0001L8.89633 5.50005Z"
      fill="currentColor"
    />
  ),
  'circle-warning': (
    <path
      transform="scale(1.6)"
      d="M7.5 5.28125V7.78125M7.5 13.125C6.76131 13.125 6.02986 12.9795 5.34741 12.6968C4.66495 12.4141 4.04485 11.9998 3.52252 11.4775C3.00019 10.9551 2.58586 10.3351 2.30318 9.65259C2.02049 8.97014 1.875 8.23869 1.875 7.5C1.875 6.76131 2.02049 6.02986 2.30318 5.34741C2.58586 4.66495 3.00019 4.04485 3.52252 3.52252C4.04485 3.00019 4.66495 2.58586 5.34741 2.30318C6.02986 2.02049 6.76131 1.875 7.5 1.875C8.99184 1.875 10.4226 2.46763 11.4775 3.52252C12.5324 4.57742 13.125 6.00816 13.125 7.5C13.125 8.99184 12.5324 10.4226 11.4775 11.4775C10.4226 12.5324 8.99184 13.125 7.5 13.125ZM7.53125 9.65625V9.71875H7.46875V9.65625H7.53125Z"
      stroke="currentColor"
      strokeWidth="1.25"
      strokeLinecap="round"
      strokeLinejoin="round"
      fill="none"
    />
  ),
  close: (
    <path
      d="M6.4 4.94 4.94 6.4 10.54 12l-5.6 5.6 1.46 1.46L12 13.46l5.6 5.6 1.46-1.46L13.46 12l5.6-5.6L17.6 4.94 12 10.54z"
      fill="currentColor"
    />
  ),
  info: (
    <path
      fillRule="evenodd"
      clipRule="evenodd"
      d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20m0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16m-1 5h2V7h-2zm0 8h2v-6h-2z"
      fill="currentColor"
    />
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M12 7V12L15.5 14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </>
  )
} as const

export type SaleIconName = keyof typeof GLYPHS

type Props = Omit<SVGProps<SVGSVGElement>, 'name'> & {
  name: SaleIconName
  /** Square size in px; without it the glyph is 20px. */
  size?: number
  color?: string
}

/** One of the modal's glyphs, drawn the way the Shop draws it. */
export function SaleIcon({ name, size, color, className, style, ...rest }: Props) {
  const sized: CSSProperties = { ...(size ? { width: size, height: size } : {}), ...(color ? { color } : {}), ...style }
  return (
    <svg
      className={cx(styles.svg, className)}
      data-sale-icon
      viewBox="0 0 24 24"
      fill="none"
      focusable="false"
      aria-hidden={rest.role ? undefined : true}
      style={sized}
      {...rest}
    >
      {GLYPHS[name]}
    </svg>
  )
}
