// MaskIcon — a white-on-transparent image used as a CSS mask, so it takes `currentColor`. The url is
// quoted: an inlined SVG is a data URI with spaces and quotes, which an unquoted url() drops.
import styles from './MaskIcon.module.css'

/** `height` makes it a `size`-wide, `height`-tall stretched image (the sign-in screens' arrow). */
export function MaskIcon({ src, size = 24, height, className }: { src: string; size?: number; height?: number; className?: string }): React.JSX.Element {
  return (
    <span
      className={`${styles.icon} ${className ?? ''}`.trim()}
      style={{ width: size, height: height ?? size, maskImage: `url("${src}")`, WebkitMaskImage: `url("${src}")` }}
      aria-hidden="true"
    />
  )
}
