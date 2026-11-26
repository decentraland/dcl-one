import type { SVGProps } from 'react'

export function PlayOnceIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
      {...props}
    >
      <path d="M20 6H7C6.20435 6 5.44129 6.31607 4.87868 6.87868C4.31607 7.44129 4 8.20435 4 9V12V15C4 16.6569 5.34315 18 7 18H17C17.7956 18 18.5587 17.6839 19.1213 17.1213C19.6839 16.5587 20 15.7956 20 15V12M17 9L20 6L17 3M11 11L12 10V14" />
    </svg>
  )
}
