// CreditsPill — the Shop header's Credits balance: the credit mark and the whole credits the buyer holds.

import { formatCredits, formatCreditsFull } from '../../../../shop/checkout/lib/currency'
import { CreditsIcon } from './CreditsIcon'
import styles from './CreditsPill.module.css'

export function CreditsPill({ credits, className }: { credits: number; className?: string }): React.JSX.Element {
  return (
    <span className={`${styles.pill} ${className ?? ''}`.trim()} role="status" aria-label={`Credits balance: ${formatCreditsFull(credits)}`}>
      <CreditsIcon size={32} />
      <span className={styles.amount}>{formatCredits(credits)}</span>
    </span>
  )
}
