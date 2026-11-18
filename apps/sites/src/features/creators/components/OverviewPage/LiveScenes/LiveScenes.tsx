import { useMemo } from 'react'
import { ChevronRightIcon } from '~/components/Icons'
import { useHotScenes } from '~/hooks/useHotScenes'
import { useTranslation } from '~/intl'
import { placeUrl, placesUrl, sceneCoordinates, selectLiveScenes } from '~/lib/hotScenes'
import { OverviewSection, trackClick } from '~/lib/overviewAnalytics'
import { AnimatedSection } from '../AnimatedSection'
import { highlight } from '../highlight'
import { cx } from '~/lib/cx'
import overview from '../OverviewPage.module.css'
import styles from './LiveScenes.module.css'

const LiveScenes = () => {
  const { t, rich } = useTranslation()
  const { data } = useHotScenes()
  const scenes = useMemo(() => selectLiveScenes(data ?? []), [data])

  // Nothing to show while loading, when the feed fails or when the world is empty.
  if (scenes.length === 0) return null

  return (
    <AnimatedSection section={OverviewSection.LIVE_SCENES}>
      <section className={styles.section} data-testid="overview-live-scenes">
        <h2 className={cx(overview.sectionTitle, styles.title)}>{rich('overview.live_scenes.title', highlight)}</h2>
        <div className={overview.rail}>
          {scenes.map(scene => (
            <a
              className={styles.sceneCard}
              key={scene.id}
              href={placeUrl(scene)}
              data-testid="overview-scene-card"
              data-place={OverviewSection.LIVE_SCENES}
              data-title={scene.name}
              onClick={trackClick}
            >
              <div className={styles.sceneImage}>
                <img src={scene.thumbnail} alt="" loading="lazy" />
                <span className={styles.onlineBadge}>
                  {t('overview.live_scenes.online', { count: scene.usersTotalCount })}
                </span>
              </div>
              <div className={styles.sceneInfo}>
                <h3 className={styles.sceneName}>{scene.name}</h3>
                <span className={styles.sceneCoords}>{sceneCoordinates(scene)}</span>
              </div>
            </a>
          ))}
        </div>
        <div className={overview.centered}>
          <a
            className={overview.viewAllLink}
            href={placesUrl()}
            data-testid="overview-live-scenes-all"
            data-place={OverviewSection.LIVE_SCENES}
            data-title="view-all"
            onClick={trackClick}
          >
            {t('overview.live_scenes.view_all')}
            <ChevronRightIcon />
          </a>
        </div>
      </section>
    </AnimatedSection>
  )
}

export { LiveScenes }
