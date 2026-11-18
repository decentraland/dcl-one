import { useCallback, useMemo, useState } from 'react'
import { useMediaQuery } from '~/hooks/useMediaQuery'
import { contentfulImage } from '~/lib/contentfulImage'
import { englishMessage, useTranslation } from '~/intl'
import { OVERVIEW_MOBILE_QUERY, OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { AnimatedSection } from '../AnimatedSection'
import { Carousel } from '../Carousel'
import { createCards, type CreateCard as CreateCardData, type CreateTab } from '../data'
import { highlight, plainText } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './CreateCards.module.css'

// Widest the figure renders: 35% of the 1200px slide on desktop, the full stacked card below `laptop`.
const FIGURE_MAX_WIDTH = 520

// The carousel renders every card three times, so DOM ids carry the slide they belong to.
type TabContentProps = { card: CreateCardData; tab: CreateTab; slide: number }

const panelId = (card: CreateCardData, tab: CreateTab, slide: number) => `create-panel-${card.id}-${tab.id}-${slide}`

const TabContent = ({ card, tab, slide }: TabContentProps) => {
  const { t } = useTranslation()
  const mobile = useMediaQuery(OVERVIEW_MOBILE_QUERY)
  const links = useMemo(() => tab.links.filter(link => !mobile || !link.desktopOnly), [tab.links, mobile])
  const prefix = `overview.create.cards.${card.id}.tabs.${tab.id}`

  return (
    <div
      className={styles.tabPanel}
      role="tabpanel"
      id={panelId(card, tab, slide)}
      data-testid="overview-create-tab-panel"
    >
      <div className={styles.infoBlock}>
        <h4 className={styles.infoTitle}>{t(`${prefix}.heading`)}</h4>
        <p className={styles.infoBody}>{t(`${prefix}.body`)}</p>
      </div>
      <div className={styles.infoBlock}>
        <h4 className={styles.infoTitle}>{t('overview.create.required_skills')}</h4>
        <div className={styles.skills}>
          {tab.skills.map(skill => (
            <span className={styles.skill} key={skill}>
              {t(`overview.create.skills.${skill}`)}
            </span>
          ))}
        </div>
      </div>
      <div className={styles.infoBlock}>
        <h4 className={styles.infoTitle}>{t('overview.create.useful_links')}</h4>
        <div className={styles.links}>
          {links.map(link => {
            const labelKey = `${prefix}.links.${link.id}`
            return (
              <a
                className={styles.link}
                key={link.id}
                href={link.url}
                target="_blank"
                rel="noopener noreferrer"
                data-testid="overview-create-link"
                data-place={OverviewSection.CREATE}
                data-card={card.analyticsCard}
                data-tab={tab.analyticsTab}
                data-title={englishMessage(labelKey)}
                onClick={trackClick}
              >
                {t(labelKey)}
              </a>
            )
          })}
        </div>
      </div>
    </div>
  )
}

const CreateCard = ({ card, slide }: { card: CreateCardData; slide: number }) => {
  const { t } = useTranslation()
  const [activeTabId, setActiveTabId] = useState(card.tabs[0].id)
  const activeTab = useMemo(
    () => card.tabs.find(tab => tab.id === activeTabId) ?? card.tabs[0],
    [card.tabs, activeTabId]
  )
  const title = t(`overview.create.cards.${card.id}.title`)

  return (
    <div className={styles.card} data-testid="overview-create-card" data-card={card.id}>
      <div
        className={styles.figure}
        style={{ backgroundImage: `url(${contentfulImage(card.background, { width: FIGURE_MAX_WIDTH })})` }}
      >
        <img src={contentfulImage(card.image, { width: FIGURE_MAX_WIDTH })} alt="" loading="lazy" />
      </div>
      <div className={styles.info}>
        <h3 className={styles.cardTitle}>{title}</h3>
        <p className={styles.cardDescription}>{t(`overview.create.cards.${card.id}.description`)}</p>
        {card.tabs.length > 1 && (
          <div className={styles.tabs} role="tablist" aria-label={title}>
            {card.tabs.map(tab => (
              <button
                className={styles.tab}
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={tab.id === activeTabId}
                aria-controls={panelId(card, tab, slide)}
                data-testid="overview-create-tab"
                data-selected={tab.id === activeTabId || undefined}
                data-place={OverviewSection.CREATE}
                data-card={card.analyticsCard}
                data-tab={tab.analyticsTab}
                onClick={event => {
                  trackClick(event)
                  setActiveTabId(tab.id)
                }}
              >
                {t(`overview.create.cards.${card.id}.tabs.${tab.id}.title`)}
              </button>
            ))}
          </div>
        )}
        <TabContent card={card} tab={activeTab} slide={slide} />
      </div>
    </div>
  )
}

const keyExtractor = (card: CreateCardData) => card.id

const CreateCards = () => {
  const { t, rich } = useTranslation()
  const renderCard = useCallback((card: CreateCardData, slide: number) => <CreateCard card={card} slide={slide} />, [])
  return (
    <AnimatedSection section={OverviewSection.CREATE}>
      <section className={styles.section} data-testid="overview-create">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.create.title', highlight)}</h2>
        <Carousel
          items={createCards}
          renderItem={renderCard}
          keyExtractor={keyExtractor}
          label={t('overview.create.title', plainText)}
          slideWidth={1200}
        />
      </section>
    </AnimatedSection>
  )
}

export { CreateCards }
