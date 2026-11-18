import { useMemo } from 'react'
import { Button } from '~/components/Button'
import { PlayIcon } from '~/components/Icons'
import { englishMessage, useTranslation } from '~/intl'
import { contentfulImage } from '~/lib/contentfulImage'
import { formatLongDate } from '~/lib/time'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { useLocale } from '~/store/locale'
import { AnimatedSection } from '../AnimatedSection'
import { SUBMIT_TUTORIAL_URL, YOUTUBE_CHANNEL_URL, learnCards, learnVideo } from '../data'
import { highlight } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './Learn.module.css'

const Learn = () => {
  const { t, rich } = useTranslation()
  const locale = useLocale(s => s.locale)
  const cards = useMemo(
    () =>
      learnCards.map(card => ({
        ...card,
        title: t(`overview.learn.cards.${card.id}`),
        analyticsTitle: englishMessage(`overview.learn.cards.${card.id}`),
        date: formatLongDate(card.date, locale),
        ...learnVideo(card.videoId)
      })),
    [t, locale]
  )

  return (
    <AnimatedSection section={OverviewSection.LEARN}>
      <section className={styles.section} data-testid="overview-learn">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.learn.title', highlight)}</h2>
        <div className={overview.rail}>
          {cards.map(card => (
            <a
              className={styles.videoCard}
              key={card.id}
              href={card.url}
              target="_blank"
              rel="noopener noreferrer"
              data-testid="overview-learn-card"
              data-place={OverviewSection.LEARN}
              data-title={card.analyticsTitle}
              onClick={trackClick}
            >
              <div className={styles.thumbnail}>
                <img src={card.thumbnail} alt="" loading="lazy" />
                <PlayIcon />
              </div>
              <div className={styles.videoInfo}>
                <div className={styles.videoMeta}>
                  <div className={styles.videoAuthor}>
                    <img
                      src={contentfulImage(card.authorImage, { width: 40 })}
                      alt=""
                      width={40}
                      height={40}
                      loading="lazy"
                    />
                    <span>{card.author}</span>
                  </div>
                  <span className={styles.videoDate}>{card.date}</span>
                </div>
                <h3 className={styles.videoTitle}>{card.title}</h3>
              </div>
            </a>
          ))}
        </div>
        <div className={styles.extras}>
          <div className={styles.extra}>
            {t('overview.learn.watch_more')}
            <Button
              as="a"
              size="compact"
              href={YOUTUBE_CHANNEL_URL}
              target="_blank"
              rel="noopener noreferrer"
              data-testid="overview-learn-youtube"
              data-place={OverviewSection.LEARN}
              data-title="watch-more"
              onClick={trackClick}
            >
              {t('overview.learn.watch_more_button')}
            </Button>
          </div>
          <div className={styles.extra}>
            {t('overview.learn.your_tutorial')}
            <Button
              as="a"
              variant="light"
              size="compact"
              href={SUBMIT_TUTORIAL_URL}
              target="_blank"
              rel="noopener noreferrer"
              data-testid="overview-learn-submit"
              data-place={OverviewSection.LEARN}
              data-title="submit-tutorial"
              onClick={trackClick}
            >
              {t('overview.learn.your_tutorial_button')}
            </Button>
          </div>
        </div>
      </section>
    </AnimatedSection>
  )
}

export { Learn }
