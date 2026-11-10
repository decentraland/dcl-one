import type { IconProps } from '../../../foundations/icons/types'

/** Three horizontal lines: the phone menu button, as source Sites' navbar. */
export const LocalMenuLines = ({ size, ...props }: IconProps) => (
  <svg
    width={size ?? 24}
    height={size ?? 24}
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    {...props}
  >
    <rect x="3" y="5" width="18" height="2" rx="1" fill="currentColor" />
    <rect x="3" y="11" width="18" height="2" rx="1" fill="currentColor" />
    <rect x="3" y="17" width="18" height="2" rx="1" fill="currentColor" />
  </svg>
)
