import styles from './MaskIcon.module.css'

export type MaskIconProps = {
  /** A white-on-transparent image. It is used as a mask, so the icon takes the text colour. */
  src: string
  /** The icon's box in pixels. 24 by default. */
  size?: number
  /** A non-square icon's box; each defaults to `size`. */
  width?: number
  height?: number
  className?: string
}

/**
 * An image drawn in `currentColor`: the image is a CSS mask over a box filled with the text colour.
 * It is decorative and hidden from assistive technology; the control it sits in carries the name.
 */
export function MaskIcon({ src, size = 24, width, height, className }: MaskIconProps) {
  // The url is quoted: an inlined SVG is a data URI with spaces and quotes, which an unquoted url() drops.
  const mask = `url("${src}")`
  return (
    <span
      className={[styles.root, className].filter(Boolean).join(' ')}
      style={{ width: width ?? size, height: height ?? size, maskImage: mask, WebkitMaskImage: mask }}
      aria-hidden="true"
    />
  )
}
