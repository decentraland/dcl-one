// One "paste this into a new tab" step: an instruction + a `chrome://` (or Windows settings) target
// with a Copy button. It's shown as copyable text, NOT a link — Chromium blocks web pages from
// navigating to chrome:// (a click or window.open is a no-op), so the user must paste it into the
// address bar themselves. Rendered inside the gate's and the GPU guide's <ol>.

import { useEffect, useState } from 'react'
import { Copy, Check } from '../../design'
import styles from './FlagStep.module.css'

export function FlagStep({ hint, target }: { hint: React.ReactNode; target: string }): React.JSX.Element {
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(false), 1200)
    return () => clearTimeout(t)
  }, [copied])
  const copy = (): void => {
    navigator.clipboard?.writeText(target).then(
      () => setCopied(true),
      () => {}
    )
  }
  return (
    <li>
      <span className={styles.stepHint}>{hint}</span>
      <div className={styles.flag}>
        <code className={styles.flagText}>{target}</code>
        <button type="button" className={styles.copy} onClick={copy} title="Copy" aria-label="Copy">
          {copied ? <Check size={15} /> : <Copy size={15} />}
        </button>
      </div>
    </li>
  )
}
