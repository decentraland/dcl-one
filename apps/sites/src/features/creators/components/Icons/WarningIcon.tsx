import type { SVGProps } from 'react'

export function WarningIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      width="48"
      height="48"
      viewBox="0 0 48 48"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
      {...props}
    >
      <path d="M21.4 6.6 4.2 36.3a3 3 0 0 0 2.6 4.5h34.4a3 3 0 0 0 2.6-4.5L26.6 6.6a3 3 0 0 0-5.2 0Z" />
      <path d="M24 17v11" />
      <circle cx="24" cy="34" r="1.75" fill="currentColor" stroke="none" />
    </svg>
  )
}
