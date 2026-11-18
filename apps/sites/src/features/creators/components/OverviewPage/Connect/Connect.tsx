import { useCallback } from 'react'
import { useTranslation } from '~/intl'
import { contentfulImage } from '~/lib/contentfulImage'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import discordIcon from '~/assets/overview/discord.svg'
import { AnimatedSection } from '../AnimatedSection'
import { Carousel } from '../Carousel'
import { DISCORD_URL, testimonials, type Testimonial } from '../data'
import { highlight, plainText } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './Connect.module.css'

const AUTOPLAY_MS = 5000

const keyExtractor = (card: Testimonial) => card.id

const Connect = () => {
  const { t, rich } = useTranslation()
  const renderCard = useCallback(
    (card: Testimonial) => (
      <a
        className={styles.card}
        href={card.url}
        target="_blank"
        rel="noopener noreferrer"
        data-testid="overview-testimonial"
        data-place={OverviewSection.CONNECT}
        data-title={card.name}
        onClick={trackClick}
      >
        <p className={styles.quote}>&ldquo;{t(`overview.connect.cards.${card.id}`)}&rdquo;</p>
        <div className={styles.author}>
          <img src={contentfulImage(card.image, { width: 40 })} alt="" width={40} height={40} loading="lazy" />
          <span className={styles.authorName}>{card.name}</span>
        </div>
      </a>
    ),
    [t]
  )

  return (
    <AnimatedSection section={OverviewSection.CONNECT}>
      <section className={styles.section} data-testid="overview-connect">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.connect.title', highlight)}</h2>
        <Carousel
          items={testimonials}
          renderItem={renderCard}
          keyExtractor={keyExtractor}
          label={t('overview.connect.title', plainText)}
          slideWidth={500}
          autoplayMs={AUTOPLAY_MS}
          alignItems="center"
        />
        <a
          className={styles.discord}
          href={DISCORD_URL}
          target="_blank"
          rel="noopener noreferrer"
          data-testid="overview-discord"
          data-place={OverviewSection.CONNECT}
          data-title="join-discord"
          onClick={trackClick}
        >
          <span className={styles.discordTitle} data-discord-title>
            {t('overview.connect.join_the_community')}
          </span>
          <span className={styles.discordIcon} data-discord-icon>
            <img src={discordIcon} alt="" />
          </span>
        </a>
      </section>
    </AnimatedSection>
  )
}

export { Connect }
