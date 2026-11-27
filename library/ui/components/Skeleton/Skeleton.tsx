import type { HTMLAttributes } from 'react'
import styles from './Skeleton.module.css'

export type SkeletonProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  /**
   * `brand` is the Figma purple sheen; `neutral` is the grey pulse decentraland.org draws on dark surfaces;
   * `overlay` is the translucent block the Explorer HUD draws over a scene or a backdrop. `sweep` and `band`
   * are the Explorer HUD's placeholders on its pages: a band of light sweeps across a translucent purple
   * block (`sweep`) or across nothing, on a dark card (`band`). Those two take their size and corners from
   * the caller's class, and read `--skeleton-sweep` (how far past each edge the band travels, 130% by
   * default) and `--skeleton-band` (the band's colour, for `band`).
   */
  tone?: 'brand' | 'neutral' | 'overlay' | 'sweep' | 'band'
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
