import { englishMessage, useTranslation } from '~/intl'
import { contentfulImage } from '~/lib/contentfulImage'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { AnimatedSection } from '../AnimatedSection'
import { whyCards } from '../data'
import { highlight } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './Why.module.css'

const Why = () => {
  const { t, rich } = useTranslation()
  return (
    <AnimatedSection section={OverviewSection.WHY}>
      <section className={styles.section} data-testid="overview-why">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.why.title', highlight)}</h2>
        <div className={styles.grid}>
          {whyCards.map(card => {
            const titleKey = `overview.why.cards.${card.id}.title`
            return (
              <a
                className={styles.card}
                key={card.id}
                href={card.url}
                target="_blank"
                rel="noopener noreferrer"
                data-testid="overview-why-card"
                data-gradient={card.gradient}
                data-place={OverviewSection.WHY}
                data-title={englishMessage(titleKey)}
                onClick={trackClick}
              >
                <div className={styles.cardInner}>
                  <div className={styles.cardImage}>
                    <img
                      src={contentfulImage(card.image.url, { width: card.image.width / 2 })}
                      alt=""
                      width={card.image.width}
                      height={card.image.height}
                      loading="lazy"
                    />
                  </div>
                  <div className={styles.cardText}>
                    <h3 className={styles.cardTitle}>{t(titleKey)}</h3>
                    <p className={styles.cardDescription}>{t(`overview.why.cards.${card.id}.description`)}</p>
                    <span className={styles.cardCta}>{t(`overview.why.cards.${card.id}.cta`)}</span>
                  </div>
                </div>
              </a>
            )
          })}
        </div>
      </section>
    </AnimatedSection>
  )
}

export { Why }
