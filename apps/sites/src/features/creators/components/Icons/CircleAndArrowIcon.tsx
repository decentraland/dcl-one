import type { SVGProps } from 'react'

/** A chevron inside a ring (the FAQ accordion toggle); the parent styles the open state. */
export function CircleAndArrowIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg width="72" height="72" viewBox="0 0 72 72" fill="none" aria-hidden focusable="false" {...props}>
      <circle cx="36" cy="36" r="35" stroke="currentColor" strokeWidth="2" opacity="0.2" />
      <path
        d="M45 33.0022L42.885 30.8872L36 37.7572L29.115 30.8872L27 33.0022L36 42.0022L45 33.0022Z"
        fill="currentColor"
      />
    </svg>
  )
}
