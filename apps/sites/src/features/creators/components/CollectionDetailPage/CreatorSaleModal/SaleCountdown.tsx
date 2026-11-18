import { useEffect, useState, type ComponentPropsWithoutRef } from 'react'
import { useTranslation } from '~/intl'
import { saleTimeLeft, formatCountdown, countdownTickMs } from '~/lib/saleCountdown'
import { SaleIcon } from './SaleIcon'
import { cx } from '~/lib/cx'
import styles from './SaleCountdown.module.css'

// Live "2d 4h" label counting down to a moment — a sale's end, or its start. Repaints on a self-adjusting
// timer — once a minute while that moment is far off, once a second in the final hour (see countdownTickMs)
// — so a whole grid of these doesn't re-render every second. Renders nothing once the moment has passed, or
// when there is no moment to count to.
export function SaleCountdown({
  until,
  className,
  testId,
  iconSize = 13,
  ...rest
}: {
  until?: number
  className?: string
  testId?: string
  iconSize?: number
  // Forwarded so a wrapper can describe it — a Tooltip hands its trigger `aria-describedby`, and a
  // component that swallows unknown props silently breaks that link.
} & ComponentPropsWithoutRef<'span'>) {
  const { t } = useTranslation()
  const [left, setLeft] = useState(() => saleTimeLeft(until))

  useEffect(() => {
    setLeft(saleTimeLeft(until))
    if (until == null) return
    let timer: ReturnType<typeof setTimeout>
    const tick = () => {
      const remaining = saleTimeLeft(until)
      setLeft(remaining)
      const next = countdownTickMs(remaining)
      if (next > 0) timer = setTimeout(tick, next)
    }
    const first = countdownTickMs(saleTimeLeft(until))
    if (first > 0) timer = setTimeout(tick, first)
    return () => clearTimeout(timer)
  }, [until])

  const label = formatCountdown(left, {
    days: t('creator_sale.unit_days'),
    hours: t('creator_sale.unit_hours'),
    minutes: t('creator_sale.unit_minutes'),
    seconds: t('creator_sale.unit_seconds')
  })
  if (!label) return null
  return (
    <span className={cx(styles.root, className)} data-testid={testId} {...rest}>
      <SaleIcon name="clock" size={iconSize} />
      {label}
    </span>
  )
}
