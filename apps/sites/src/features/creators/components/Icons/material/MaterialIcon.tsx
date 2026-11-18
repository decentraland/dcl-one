import type { SVGProps } from 'react'
import { cx } from '~/lib/cx'
import styles from './MaterialIcon.module.css'

export type MaterialIconProps = Omit<SVGProps<SVGSVGElement>, 'fontSize'> & {
  fontSize?: 'small' | 'medium' | 'large' | 'inherit'
}

/** Renders a Material glyph as MUI's SvgIcon did: a 1em square in `currentColor`, sized by `fontSize`. */
export function MaterialIcon({ fontSize = 'medium', className, children, ...props }: MaterialIconProps) {
  return (
    <svg
      className={cx(styles.root, className)}
      data-font-size={fontSize}
      viewBox="0 0 24 24"
      focusable="false"
      aria-hidden
      {...props}
    >
      {children}
    </svg>
  )
}
