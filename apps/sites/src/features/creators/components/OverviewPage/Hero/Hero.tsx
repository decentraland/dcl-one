import { type MouseEvent, useEffect, useMemo, useState } from 'react'
import { Button } from '~/components/Button'
import { Tooltip } from '~/components/Tooltip'
import { ChevronDownIcon, AppleIcon, MicrosoftIcon } from '~/components/Icons'
import { useAfterLoadIdle } from '~/hooks/useAfterLoadIdle'
import { useCreatorHubDownload } from '~/hooks/useCreatorHubDownload'
import { useMediaQuery } from '~/hooks/useMediaQuery'
import { useTypingListEffect } from '~/hooks/useTypingListEffect'
import { useTranslation } from '~/intl'
import {
  buildName,
  cancelCreatorHubRedirect,
  type CreatorHubDownload,
  type DownloadOption,
  startCreatorHubDownload
} from '~/lib/creatorHubDownload'
import {
  CREATOR_HUB_TARGET,
  DOWNLOAD_CLICK,
  OVERVIEW_MOBILE_QUERY,
  OverviewSection,
  trackClick
} from '~/lib/overviewAnalytics'
import { CREATOR_DOCS_URL, CREATOR_HUB_DOWNLOAD_URL, heroData } from '../data'
import styles from './Hero.module.css'

const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)'

const prefersSavingData = () =>
  (navigator as Navigator & { connection?: { saveData?: boolean } }).connection?.saveData === true

/** Its own component so the fade-in state starts over on every mount. */
const HeroVideo = () => {
  const [playing, setPlaying] = useState(false)
  const { url, width, height } = heroData.video
  return (
    <video
      autoPlay
      loop
      muted
      playsInline
      width={width}
      height={height}
      aria-hidden
      data-testid="overview-hero-video"
      data-playing={playing || undefined}
      onPlaying={() => setPlaying(true)}
    >
      <source src={url} type="video/mp4" />
    </video>
  )
}

const osIcons = { Windows: MicrosoftIcon, macOS: AppleIcon }

const DownloadOsIcon = ({ os }: { os: CreatorHubDownload['os'] }) => {
  const Icon = osIcons[os]
  return <Icon aria-hidden />
}

// A modifier click opens the installer elsewhere, so this tab stays put; a repeat click within the redirect
// window would download it twice.
function onDownloadClick(
  event: MouseEvent<HTMLAnchorElement>,
  target: CreatorHubDownload | undefined,
  option: DownloadOption
) {
  trackClick(event)
  const modified = event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
  if (target && !modified && !startCreatorHubDownload(target, option)) event.preventDefault()
}

const Hero = () => {
  const { t } = useTranslation()
  const mobile = useMediaQuery(OVERVIEW_MOBILE_QUERY)
  const words = useMemo(() => heroData.words.map(word => t(`overview.hero.words.${word}`)), [t])
  const currentWord = useTypingListEffect(words)
  const { poster } = heroData
  const reducedMotion = useMediaQuery(REDUCED_MOTION_QUERY)
  const idle = useAfterLoadIdle()
  // After the first paint, and never on phones: they get the docs, and the GitHub API is rate limited per IP.
  const { download, others = [], fallback } = useCreatorHubDownload(!mobile && idle)
  useEffect(() => cancelCreatorHubRedirect, [])
  const [posterFailed, setPosterFailed] = useState(false)
  // The poster is the first paint and the LCP; the video only joins once the page is idle, and only on
  // desktop, where its weight is affordable.
  const showVideo = !mobile && idle && !reducedMotion && !prefersSavingData()

  return (
    <>
      <section className={styles.hero} data-testid="overview-hero">
        <div className={styles.background}>
          {!posterFailed && (
            <img
              src={poster.url}
              srcSet={poster.srcSet}
              sizes="100vw"
              alt=""
              width={poster.width}
              height={poster.height}
              data-testid="overview-hero-poster"
              onError={() => setPosterFailed(true)}
            />
          )}
          {showVideo && <HeroVideo />}
        </div>
        <div className={styles.content}>
          <h1 className={styles.title}>
            {t('overview.hero.title_first_line')}
            <br />
            <span data-testid="overview-hero-word">{currentWord}</span>
            <br />
            {t('overview.hero.title_last_line')}
          </h1>
          <p className={styles.subtitle}>{t('overview.hero.subtitle')}</p>
          {mobile ? (
            // The Creator Hub only ships desktop installers, so phones get the docs instead of a dead end.
            <Button
              as="a"
              size="hero"
              href={CREATOR_DOCS_URL}
              onClick={trackClick}
              data-testid="overview-hero-cta"
              data-place={OverviewSection.HERO}
            >
              {t('overview.hero.mobile_docs_cta')}
            </Button>
          ) : (
            <>
              <Tooltip
                content={download ? t(`overview.hero.download_for.${buildName(download)}`) : null}
                asChild
                describeChild
                testId="overview-hero-cta-tooltip"
              >
                <Button
                  as="a"
                  size="hero"
                  // Until the release resolves, or when no installer fits this OS, the download page offers them all.
                  href={download?.href ?? CREATOR_HUB_DOWNLOAD_URL}
                  onClick={(event: MouseEvent<HTMLAnchorElement>) => onDownloadClick(event, download, 'primary')}
                  data-testid="overview-hero-cta"
                  data-place={OverviewSection.HERO}
                  data-event={DOWNLOAD_CLICK}
                  data-os={download?.os}
                  data-arch={download?.arch}
                  data-download-target={CREATOR_HUB_TARGET}
                  data-download-mode={download ? 'direct' : fallback}
                  data-download-option="primary"
                >
                  {t('overview.hero.download_cta')}
                  {download && (
                    <span className={styles.osIcon} data-testid="overview-hero-os-icon" data-os={download.os}>
                      <DownloadOsIcon os={download.os} />
                    </span>
                  )}
                </Button>
              </Tooltip>
              {others.length > 0 && (
                <div className={styles.alsoAvailable} data-testid="overview-hero-also-available">
                  {t('overview.hero.also_available')}
                  {others.map(other => {
                    const name = buildName(other)
                    return (
                      <Tooltip key={name} content={t(`overview.hero.download_for.${name}`)} asChild>
                        <a
                          className={styles.altDownload}
                          href={other.href}
                          aria-label={t(`overview.hero.download_for.${name}`)}
                          onClick={event => onDownloadClick(event, other, 'alternative')}
                          data-testid="overview-hero-alt-download"
                          data-place={OverviewSection.HERO}
                          data-event={DOWNLOAD_CLICK}
                          data-os={other.os}
                          data-arch={other.arch}
                          data-download-target={CREATOR_HUB_TARGET}
                          data-download-mode="direct"
                          data-download-option="alternative"
                        >
                          <DownloadOsIcon os={other.os} />
                          {other.os === 'macOS' && <span>{t(`overview.hero.build_label.${name}`)}</span>}
                        </a>
                      </Tooltip>
                    )
                  })}
                </div>
              )}
            </>
          )}
        </div>
      </section>
      <button
        className={styles.scrollButton}
        type="button"
        aria-label={t('overview.hero.scroll_label')}
        data-testid="overview-hero-scroll"
        data-place={OverviewSection.HERO}
        data-title="scroll-to-why"
        onClick={event => {
          trackClick(event)
          window.scrollBy({ top: window.innerHeight, behavior: 'smooth' })
        }}
      >
        <ChevronDownIcon />
      </button>
    </>
  )
}

export { Hero }
