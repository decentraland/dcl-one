import type { ButtonHTMLAttributes, ReactNode } from 'react'
import styles from './CardActionButton.module.css'

export type CardActionButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
  /** The button has no visible text, so it needs an accessible name. */
  'aria-label': string
  /** The 24px glyph; the Figma thumbs-up when not given. */
  icon?: ReactNode
}

function ThumbIcon() {
  return (
    <svg className={styles.icon} viewBox="0 0 24 24" fill="none" focusable="false" aria-hidden="true">
      <path
        className={styles.iconStroke}
        strokeWidth="2"
        d="M4.00018 10.9822C3.99636 10.6941 4.05394 10.4082 4.16939 10.1422C4.28484 9.8762 4.45572 9.63567 4.67154 9.43538L10.7944 3.53169C11.2509 3.2481 12.2714 4.9496 12.2714 5.38787C12.2714 5.82614 11.224 9.61585 11.224 9.61585C11.22 9.63479 11.2205 9.65436 11.2254 9.67312C11.2303 9.69187 11.2396 9.70932 11.2524 9.72417C11.2653 9.73903 11.2816 9.75091 11.2999 9.75893C11.3182 9.76696 11.3382 9.77092 11.3583 9.77053C13.1286 9.77053 15.8079 9.75344 17.6397 9.74001C18.751 9.73187 19.5953 10.6351 19.2576 11.6939C18.6076 13.7321 17.3006 16.4145 16.407 18.5155C16.2415 18.9046 15.9589 19.2375 15.5954 19.4717C15.2318 19.7059 14.8038 19.8307 14.366 19.8303H6.00018C4.89561 19.8303 4.00018 18.9359 4.00018 17.8313C4.00018 15.6481 4.00018 12.2883 4.00018 10.9822Z"
      />
    </svg>
  )
}

export function CardActionButton({ icon, className, type = 'button', ...rest }: CardActionButtonProps) {
  return (
    <button {...rest} type={type} className={[styles.root, className].filter(Boolean).join(' ')}>
      {icon ?? <ThumbIcon />}
    </button>
  )
}
