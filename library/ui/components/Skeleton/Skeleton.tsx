import type { HTMLAttributes } from 'react'
import styles from './Skeleton.module.css'

export type SkeletonProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  /**
   * `brand` is the Figma purple sheen; `neutral` is the grey pulse decentraland.org draws on dark surfaces;
   * `overlay` is the translucent block the Explorer HUD draws over a scene or a backdrop.
   */
  tone?: 'brand' | 'neutral' | 'overlay'
}

/**
 * A loading placeholder: a purple block with a sheen that sweeps across it. It is decoration, so
 * assistive technology skips it; the region that loads states its busy state (`aria-busy`).
 * The consumer sizes it through `className` or `style`; without either it takes the Figma 200 × 112.
 */
export function Skeleton({ className, tone = 'brand', ...rest }: SkeletonProps) {
  const classes = [styles.root, tone !== 'brand' && styles[tone], className].filter(Boolean).join(' ')
  return <span aria-hidden="true" {...rest} className={classes} />
}
