import { useMemo, useState, type KeyboardEvent, type SyntheticEvent } from 'react'
import { useInView } from 'react-intersection-observer'
import { Button } from '~/components/Button'
import { CircleAndArrowIcon } from '~/components/Icons'
import { useSectionViewed } from '~/hooks/useSectionViewed'
import { englishMessage, useTranslation } from '~/intl'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { FAQS_URL, faqIds } from '../data'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './Faqs.module.css'

type FaqRowProps = { id: string; open: boolean; onToggle: () => void }

const FaqRow = ({ id, open, onToggle }: FaqRowProps) => {
  const { t } = useTranslation()
  const questionKey = `overview.faqs.items.${id}.question`

  const toggle = (event: SyntheticEvent<HTMLElement>) => {
    // Only the expand is an engagement signal.
    if (!open) trackClick(event)
    onToggle()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      toggle(event)
    }
  }

  return (
    <div
      className={styles.row}
      role="button"
      tabIndex={0}
      aria-expanded={open}
      aria-controls={`faq-answer-${id}`}
      data-testid="overview-faq"
      data-open={open || undefined}
      data-place={OverviewSection.FAQS}
      data-title={englishMessage(questionKey)}
      onClick={toggle}
      onKeyDown={onKeyDown}
    >
      <div className={styles.question}>
        <h3 className={styles.questionText} id={`faq-question-${id}`}>
          {t(questionKey)}
        </h3>
        <CircleAndArrowIcon />
      </div>
      <div
        className={styles.answer}
        id={`faq-answer-${id}`}
        role="region"
        aria-labelledby={`faq-question-${id}`}
        aria-hidden={!open}
      >
        <p className={styles.answerText}>{t(`overview.faqs.items.${id}.answer`)}</p>
      </div>
    </div>
  )
}

const Faqs = () => {
  const { t } = useTranslation()
  const [openId, setOpenId] = useState<string | null>(null)
  // No reveal animation here, so the section observes the viewport itself for `Section Viewed`.
  const { ref, inView } = useInView({ triggerOnce: true, threshold: 0.1 })
  useSectionViewed(OverviewSection.FAQS, inView)
  const structuredData = useMemo(
    () =>
      JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'FAQPage',
        mainEntity: faqIds.map(id => ({
          '@type': 'Question',
          name: t(`overview.faqs.items.${id}.question`),
          acceptedAnswer: { '@type': 'Answer', text: t(`overview.faqs.items.${id}.answer`) }
        }))
      }),
    [t]
  )

  return (
    <section
      className={styles.section}
      ref={ref}
      aria-label={t('overview.faqs.aria_label')}
      data-testid="overview-faqs"
    >
      <script type="application/ld+json" data-testid="overview-faqs-structured-data">
        {structuredData}
      </script>
      <div className={styles.frame}>
        <div className={styles.container}>
          <p className={styles.subtitle}>{t('overview.faqs.subtitle')}</p>
          <h2 className={cx(overview.sectionTitle, styles.title)}>{t('overview.faqs.title')}</h2>
          {faqIds.map(id => (
            <FaqRow key={id} id={id} open={openId === id} onToggle={() => setOpenId(openId === id ? null : id)} />
          ))}
          <div className={styles.cta}>
            <Button
              as="a"
              variant="ghost"
              size="lg"
              href={FAQS_URL}
              data-testid="overview-faqs-cta"
              data-place={OverviewSection.FAQS}
              data-title="faqs-cta"
              onClick={trackClick}
            >
              {t('overview.faqs.cta')}
            </Button>
          </div>
        </div>
      </div>
    </section>
  )
}

export { Faqs }
