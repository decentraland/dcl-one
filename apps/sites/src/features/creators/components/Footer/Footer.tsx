import { useEffect, useRef, useState } from 'react'
import { useInView } from 'react-intersection-observer'
import { useLocation } from 'react-router-dom'
import { useAfterLoadIdle } from '~/hooks/useAfterLoadIdle'
import { useLocale, LOCALES, type Locale } from '~/store/locale'
import { englishMessage, useTranslation } from '~/intl'
import { isOverviewPath, LandingPlace, trackLandingClick } from '~/lib/overviewAnalytics'
import styles from './Footer.module.css'

// The full Decentraland footer — the large purple footer with newsletter, MENU columns, social links
// and the dark legal bar. Ported from shop's ShopFooter.

const BEEHIIV_EMBED_URL = 'https://embeds.beehiiv.com/ff89783d-748b-4ba3-8700-4759f6f62831?slim=true'

// `label` holds an i18n key (translated at render) so the footer follows the active locale.
const gettingStartedLinks = [
  { label: 'footer.linkWhatIs', url: 'https://docs.decentraland.org/introduction/about-decentraland' },
  { label: 'footer.linkDownload', url: 'https://decentraland.org/download' },
  { label: 'footer.linkSystemRequirements', url: 'https://docs.decentraland.org/in-world/settings-and-performance' },
  { label: 'footer.linkFaqs', url: 'https://docs.decentraland.org/faqs/decentraland-101' },
  { label: 'footer.linkContactSupport', url: 'https://decentraland.org/help/' }
]

const resourceLinks = [
  { label: 'footer.linkMarketplace', url: 'https://decentraland.org/marketplace' },
  { label: 'footer.linkCreatorHub', url: 'https://decentraland.org/create/' },
  { label: 'footer.linkDocs', url: 'https://docs.decentraland.org' },
  { label: 'footer.linkBlog', url: 'https://decentraland.org/blog/' },
  { label: 'footer.linkVote', url: 'https://decentraland.org/dao' }
]

const socialLinks = [
  { name: 'Discord', url: 'https://dcl.gg/discord', icon: Discord },
  { name: 'GitHub', url: 'https://github.com/decentraland', icon: GitHub },
  { name: 'X', url: 'https://x.com/decentraland', icon: XTwitter },
  { name: 'Instagram', url: 'https://instagram.com/decentraland_foundation/', icon: Instagram },
  { name: 'YouTube', url: 'https://youtube.com/@decentraland_foundation', icon: YouTube },
  { name: 'TikTok', url: 'https://tiktok.com/@decentraland_fdn', icon: TikTok },
  { name: 'LinkedIn', url: 'https://linkedin.com/company/decentralandorg', icon: LinkedIn }
]

const legalLinks = [
  { label: 'footer.linkPrivacy', url: 'https://decentraland.org/privacy/' },
  { label: 'footer.linkTerms', url: 'https://decentraland.org/terms/' },
  { label: 'footer.linkContentPolicy', url: 'https://decentraland.org/content/' },
  { label: 'footer.linkEthics', url: 'https://decentraland.org/ethics/' }
]

const LANGUAGE_LABELS: Record<Locale, { label: string; flag: string }> = {
  en: { label: 'English', flag: '🇺🇸' },
  es: { label: 'Español', flag: '🇪🇸' },
  zh: { label: '中文', flag: '🇨🇳' }
}

const SocialRow = ({ onTrack }: { onTrack: (platform: string) => void }) => {
  return (
    <div className={styles.social}>
      {socialLinks.map(({ name, url, icon: Icon }) => (
        <a
          key={name}
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          aria-label={name}
          data-testid="footer-social-link"
          onClick={() => onTrack(name)}
        >
          <Icon />
        </a>
      ))}
    </div>
  )
}

const Footer = () => {
  const { t } = useTranslation()
  // Only the overview stands in for sites' /create; its labels go out in English whatever the locale.
  const onOverview = isOverviewPath(useLocation().pathname)
  const trackLink = (labelKey: string) => {
    if (onOverview) trackLandingClick(LandingPlace.FOOTER_LINK, { link: englishMessage(labelKey) })
  }
  const trackSocial = (platform: string) => {
    if (onOverview) trackLandingClick(LandingPlace.FOOTER_SOCIAL, { platform })
  }
  const locale = useLocale(s => s.locale)
  const setLocale = useLocale(s => s.setLocale)
  const [openSection, setOpenSection] = useState<string | null>(null)
  const [langOpen, setLangOpen] = useState(false)
  const langRef = useRef<HTMLDivElement>(null)
  const langBtnRef = useRef<HTMLButtonElement>(null)
  const current = LANGUAGE_LABELS[locale] ?? LANGUAGE_LABELS.en
  // The embed pulls in its own scripts, fonts and bot checks: it only loads when a visitor nears the footer.
  // Watching starts once the page has settled, since the footer sits by the fold until the sections stream in.
  const idle = useAfterLoadIdle()
  const { ref: newsRef, inView: newsNear } = useInView({
    skip: !idle,
    triggerOnce: true,
    rootMargin: '400px 0px',
    fallbackInView: true
  })

  useEffect(() => {
    if (!langOpen) return
    const onDown = (e: MouseEvent) => {
      if (langRef.current && !langRef.current.contains(e.target as Node)) setLangOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setLangOpen(false)
        langBtnRef.current?.focus()
      }
    }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [langOpen])

  const toggle = (key: string) => setOpenSection(prev => (prev === key ? null : key))

  return (
    <footer className={styles.footer}>
      <div className={styles.main}>
        <div className={styles.left}>
          <span className={styles.wordmark}>Decentraland</span>

          <div className={styles.news} ref={newsRef}>
            <p className={styles.newsTitle}>{t('footer.newsletterTitle')}</p>
            <iframe
              className={styles.newsFrame}
              src={newsNear ? BEEHIIV_EMBED_URL : undefined}
              // Cross-origin embed: it may run and submit its form, never navigate this page or open unscoped windows.
              sandbox="allow-scripts allow-forms allow-same-origin allow-popups"
              data-testid="footer-newsletter-frame"
              height="65"
              frameBorder="0"
              scrolling="no"
              title={t('footer.newsletterFrameTitle')}
            />
          </div>

          <div className={styles.connect} data-variant="desktop">
            <p className={styles.label}>{t('footer.connect')}</p>
            <SocialRow onTrack={trackSocial} />
          </div>
        </div>

        <div className={styles.right}>
          <div className={styles.col}>
            <p className={styles.label}>{t('footer.gettingStarted')}</p>
            {gettingStartedLinks.map(l => (
              <a
                className={styles.footLink}
                key={l.label}
                href={l.url}
                target="_blank"
                rel="noopener noreferrer"
                data-testid="footer-link"
                onClick={() => trackLink(l.label)}
              >
                {t(l.label)}
              </a>
            ))}
          </div>
          <div className={styles.col}>
            <p className={styles.label}>{t('footer.resources')}</p>
            {resourceLinks.map(l => (
              <a
                className={styles.footLink}
                key={l.label}
                href={l.url}
                target="_blank"
                rel="noopener noreferrer"
                data-testid="footer-link"
                onClick={() => trackLink(l.label)}
              >
                {t(l.label)}
              </a>
            ))}
          </div>
        </div>

        {/* Mobile-only collapsible menu. */}
        <div className={styles.mobileMenu}>
          <p className={styles.menuLabel}>{t('footer.menu')}</p>
          {[
            { key: 'getting-started', label: t('footer.gettingStarted'), links: gettingStartedLinks },
            { key: 'resources', label: t('footer.resources'), links: resourceLinks }
          ].map(section => (
            <div key={section.key}>
              <button
                className={styles.dropdown}
                type="button"
                aria-expanded={openSection === section.key}
                aria-controls={`footer-section-${section.key}`}
                data-testid={`footer-section-toggle-${section.key}`}
                onClick={() => toggle(section.key)}
              >
                {section.label}
                <span className={styles.chev} data-open={openSection === section.key || undefined}>
                  <ChevronDown />
                </span>
              </button>
              <div
                className={styles.dropContent}
                id={`footer-section-${section.key}`}
                data-open={openSection === section.key || undefined}
              >
                {section.links.map(l => (
                  <a
                    className={styles.mLink}
                    key={l.label}
                    href={l.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    onClick={() => trackLink(l.label)}
                  >
                    {t(l.label)}
                  </a>
                ))}
              </div>
            </div>
          ))}
        </div>

        <div className={styles.connect} data-variant="mobile">
          <p className={styles.label}>{t('footer.connect')}</p>
          <SocialRow onTrack={trackSocial} />
        </div>
      </div>

      <div className={styles.bottom}>
        <div className={styles.bottomLeft}>
          <div className={styles.lang} ref={langRef}>
            <button
              className={styles.langBtn}
              ref={langBtnRef}
              type="button"
              aria-expanded={langOpen}
              onClick={() => setLangOpen(o => !o)}
            >
              <span aria-hidden>{current.flag}</span>
              {current.label}
              <span className={styles.chev} data-open={langOpen || undefined}>
                <ChevronDown />
              </span>
            </button>
            {langOpen ? (
              <div className={styles.langMenu}>
                {LOCALES.map(code => (
                  <button
                    key={code}
                    data-active={code === locale || undefined}
                    onClick={() => {
                      setLocale(code)
                      setLangOpen(false)
                      langBtnRef.current?.focus()
                    }}
                  >
                    <span aria-hidden>{LANGUAGE_LABELS[code].flag}</span>
                    {LANGUAGE_LABELS[code].label}
                  </button>
                ))}
              </div>
            ) : null}
          </div>

          <div className={styles.legal}>
            {legalLinks.map(l => (
              <a className={styles.legalLink} key={l.label} href={l.url} target="_blank" rel="noopener noreferrer">
                {t(l.label)}
              </a>
            ))}
          </div>
        </div>

        <span className={styles.copy}>&copy; {new Date().getFullYear()} Decentraland</span>
      </div>
    </footer>
  )
}

export { Footer }

/* Inline SVG icons (no external deps). */

function Discord() {
  return (
    <svg viewBox="0 0 28 22" fill="currentColor" width="26" height="20" aria-hidden>
      <path d="M23.7 1.84A23.25 23.25 0 0 0 17.96 0c-.25.45-.54 1.05-.74 1.53a21.5 21.5 0 0 0-6.45 0C10.57 1.05 10.27.45 10.02 0A23.2 23.2 0 0 0 4.27 1.85C.62 7.34-.38 12.7.12 18a23.37 23.37 0 0 0 7.13 3.6 17.4 17.4 0 0 0 1.53-2.49 15.17 15.17 0 0 1-2.41-1.16c.2-.15.4-.3.59-.46a16.63 16.63 0 0 0 14.1 0c.19.16.39.31.58.46-.77.45-1.57.84-2.41 1.16.44.87.96 1.7 1.53 2.49a23.3 23.3 0 0 0 7.14-3.61c.58-6.15-1-11.47-4.1-16.15ZM9.35 14.74c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Zm9.38 0c-1.4 0-2.54-1.28-2.54-2.85s1.12-2.86 2.54-2.86 2.56 1.29 2.54 2.86c0 1.57-1.12 2.85-2.54 2.85Z" />
    </svg>
  )
}
function GitHub() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24" aria-hidden>
      <path d="M12 .3a12 12 0 0 0-3.8 23.38c.6.12.82-.26.82-.58v-2.02c-3.34.73-4.04-1.61-4.04-1.61-.55-1.39-1.34-1.76-1.34-1.76-1.08-.74.08-.73.08-.73 1.2.09 1.84 1.24 1.84 1.24 1.07 1.83 2.8 1.3 3.49 1 .1-.78.42-1.3.76-1.6-2.67-.31-5.47-1.34-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.13-.3-.54-1.52.12-3.18 0 0 1-.33 3.3 1.23a11.5 11.5 0 0 1 6.02 0c2.28-1.56 3.29-1.23 3.29-1.23.66 1.66.25 2.88.12 3.18a4.65 4.65 0 0 1 1.24 3.22c0 4.61-2.81 5.62-5.48 5.92.43.37.81 1.1.81 2.22v3.29c0 .32.22.7.82.58A12 12 0 0 0 12 .3Z" />
    </svg>
  )
}
function XTwitter() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="22" height="22" aria-hidden>
      <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231 5.45-6.231Zm-1.161 17.52h1.833L7.084 4.126H5.117L17.083 19.77Z" />
    </svg>
  )
}
function Instagram() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24" aria-hidden>
      <path d="M12 2.163c3.204 0 3.584.012 4.85.07 3.252.148 4.771 1.691 4.919 4.919.058 1.265.069 1.645.069 4.849 0 3.205-.012 3.584-.069 4.849-.149 3.225-1.664 4.771-4.919 4.919-1.266.058-1.644.07-4.85.07-3.204 0-3.584-.012-4.849-.07-3.26-.149-4.771-1.699-4.919-4.92-.058-1.265-.07-1.644-.07-4.849 0-3.204.013-3.583.07-4.849.149-3.227 1.664-4.771 4.919-4.919 1.266-.057 1.645-.069 4.849-.069ZM12 0C8.741 0 8.333.014 7.053.072 2.695.272.273 2.69.073 7.052.014 8.333 0 8.741 0 12c0 3.259.014 3.668.072 4.948.2 4.358 2.618 6.78 6.98 6.98C8.333 23.986 8.741 24 12 24c3.259 0 3.668-.014 4.948-.072 4.354-.2 6.782-2.618 6.979-6.98.059-1.28.073-1.689.073-4.948 0-3.259-.014-3.667-.072-4.947-.196-4.354-2.617-6.78-6.979-6.98C15.668.014 15.259 0 12 0Zm0 5.838a6.162 6.162 0 1 0 0 12.324 6.162 6.162 0 0 0 0-12.324ZM12 16a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm6.406-11.845a1.44 1.44 0 1 0 0 2.881 1.44 1.44 0 0 0 0-2.881Z" />
    </svg>
  )
}
function YouTube() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24" aria-hidden>
      <path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.546 12 3.546 12 3.546s-7.505 0-9.377.504A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.504 9.376.504 9.376.504s7.505 0 9.377-.504a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814ZM9.545 15.568V8.432L15.818 12l-6.273 3.568Z" />
    </svg>
  )
}
function TikTok() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="22" height="22" aria-hidden>
      <path d="M12.525.02c1.31-.02 2.61-.01 3.91-.02.08 1.53.63 3.09 1.75 4.17 1.12 1.11 2.7 1.62 4.24 1.79v4.03c-1.44-.05-2.89-.35-4.2-.97-.57-.26-1.1-.59-1.62-.93-.01 2.92.01 5.84-.02 8.75-.08 1.4-.54 2.79-1.35 3.94-1.31 1.92-3.58 3.17-5.91 3.21-1.43.08-2.86-.31-4.08-1.03-2.02-1.19-3.44-3.37-3.65-5.71-.02-.5-.03-1-.01-1.49.18-1.9 1.12-3.72 2.58-4.96 1.66-1.44 3.98-2.13 6.15-1.72.02 1.48-.04 2.96-.04 4.44-.99-.32-2.15-.23-3.02.37-.63.41-1.11 1.04-1.36 1.75-.21.51-.15 1.07-.14 1.61.24 1.64 1.82 3.02 3.5 2.87 1.12-.01 2.19-.66 2.77-1.61.19-.33.4-.67.41-1.06.1-1.79.06-3.57.07-5.36.01-4.03-.01-8.05.02-12.07Z" />
    </svg>
  )
}
function LinkedIn() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24" aria-hidden>
      <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286ZM5.337 7.433a2.062 2.062 0 1 1 0-4.123 2.062 2.062 0 0 1 0 4.123ZM6.863 20.452H3.804V9h3.059v11.452ZM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003Z" />
    </svg>
  )
}
function ChevronDown() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18" aria-hidden>
      <path d="M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41Z" />
    </svg>
  )
}
