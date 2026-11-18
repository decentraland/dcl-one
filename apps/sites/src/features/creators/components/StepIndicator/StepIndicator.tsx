import { useTranslation } from '~/intl'
import styles from './StepIndicator.module.css'

type Props = {
  /** 1-based current step. */
  current: number
  total: number
  /** One caption per step, shown under its dot. */
  labels?: string[]
  testId?: string
}

/** Numbered dots joined by a line; done steps and the line up to the current one paint red. */
export function StepIndicator({ current, total, labels, testId = 'steps' }: Props) {
  const { t } = useTranslation()
  return (
    <ol
      className={styles.steps}
      aria-label={t('step_indicator.label', { step: current, total })}
      data-labelled={labels ? '' : undefined}
      data-testid={testId}
    >
      {Array.from({ length: total }, (_, index) => {
        const step = index + 1
        const state = step < current ? 'done' : step === current ? 'current' : 'todo'
        const align = index === 0 ? 'start' : index === total - 1 ? 'end' : 'center'
        return (
          <li
            className={styles.stepNode}
            key={step}
            data-reached={step < current || undefined}
            aria-current={step === current || undefined}
          >
            <span className={styles.stepDot} data-state={state} data-testid={`${testId}-${step}`}>
              {step}
            </span>
            {labels?.[index] && (
              <span className={styles.stepLabel} data-align={align}>
                {labels[index]}
              </span>
            )}
          </li>
        )
      })}
    </ol>
  )
}
