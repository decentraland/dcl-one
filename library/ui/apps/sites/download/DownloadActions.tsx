import type { MouseEvent, ReactNode } from 'react'
import { Button } from '../shared/Button/Button'
import { GeneralApple, GeneralVerified, GeneralWindows } from '../../../foundations/icons'
import appStoreBadge from '../assets/download-on-the-app-store.svg'
import epicIcon from '../assets/epic_icon.svg'
import googlePlayBadge from '../assets/google_play_cta.svg'
import googlePlayIcon from '../assets/google_play_icon.svg'
import iosIcon from '../assets/ios-logo.svg'
import styles from './DownloadActions.module.css'

type DownloadAppearance = 'default' | 'source'

type Click = (event: MouseEvent<HTMLElement>) => void
type DesktopOs = 'windows' | 'macos'
type AlternativeIcon = DesktopOs | 'ios' | 'android'

/** An alternative platform: a link to its download, or a button (e.g. opening a QR dialog). */
type DownloadAlternative = {
  icon: AlternativeIcon
  label: string
  onClick: Click
} & ({ href: string; external?: boolean } | { href?: undefined; external?: undefined })

type DownloadActionsProps = {
  /** Opt into the measured landing CTA geometry without changing other download contexts. */
  appearance?: DownloadAppearance
  /** The installer for the visitor's OS; absent when none can run there. */
  primary?: { label: ReactNode; os: DesktopOs; osName: string; href: string; onClick: Click }
  epic?: { label: ReactNode; iconLabel: string; href: string; onClick: Click }
  /** The download counter line, e.g. "Total Downloads: +400K". */
  downloads?: ReactNode
  /** Other platforms as icon-only controls, each named by `label`. */
  alternatives?: readonly DownloadAlternative[]
  /** `center` for centered layouts such as the home hero; rows start-align by default. */
  align?: 'start' | 'center'
}

function OsIcon({ os, label, size = 32 }: { os: DesktopOs; label?: string; size?: number }) {
  const a11y = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true }
  return os === 'windows' ? (
    <GeneralWindows size={size} {...a11y} />
  ) : (
    <GeneralApple size={size} {...a11y} />
  )
}

function AlternativeImage({ icon, source }: { icon: AlternativeIcon; source: boolean }) {
  if (icon === 'windows' || icon === 'macos') return <OsIcon os={icon} size={source ? 24 : 32} />
  return (
    <img
      className={source ? styles.sourceAltIcon : styles.altIcon}
      src={icon === 'ios' ? iosIcon : googlePlayIcon}
      alt=""
    />
  )
}

/** The desktop download CTAs shared by /download, the home hero and Come Hang Out. */
function DownloadActions({
  primary,
  epic,
  downloads,
  alternatives = [],
  align = 'start',
  appearance = 'default'
}: DownloadActionsProps) {
  const centered = align === 'center'
  const source = appearance === 'source'
  const actionClass = source ? styles.sourceAction : undefined
  const alternativeClass = source ? styles.sourceAlternative : undefined
  const row = (base?: string) => [base, centered && styles.centeredRow].filter(Boolean).join(' ')
  return (
    <div className={centered ? `${styles.root} ${styles.centered}` : styles.root}>
      {primary || epic ? (
        <div
          className={[row(styles.actions), source && styles.sourceActions]
            .filter(Boolean)
            .join(' ')}
        >
          {primary ? (
            <Button className={actionClass} href={primary.href} onClick={primary.onClick}>
              {/* The space keeps the accessible name "Download for Windows", not "forWindows". */}
              {primary.label} <OsIcon os={primary.os} label={primary.osName} />
            </Button>
          ) : null}
          {epic ? (
            <Button
              className={source ? `${styles.sourceAction} ${styles.sourceEpic}` : undefined}
              variant="inverse"
              href={epic.href}
              target="_blank"
              onClick={epic.onClick}
            >
              {epic.label}{' '}
              <img
                className={source ? styles.sourceEpicIcon : styles.epic}
                src={epicIcon}
                alt={epic.iconLabel}
              />
            </Button>
          ) : null}
        </div>
      ) : null}

      {downloads || alternatives.length ? (
        <div className={row(styles.alternatives)}>
          {downloads ? (
            <p
              className={[styles.downloads, source && styles.sourceDownloads]
                .filter(Boolean)
                .join(' ')}
            >
              <GeneralVerified size={20} aria-hidden="true" />
              {downloads}
            </p>
          ) : null}
          {alternatives.length ? (
            <ul className={styles.altList}>
              {alternatives.map((alt) => (
                <li key={alt.icon}>
                  {alt.href !== undefined ? (
                    <Button
                      className={alternativeClass}
                      variant="iconOnly"
                      aria-label={alt.label}
                      href={alt.href}
                      onClick={alt.onClick}
                      {...(alt.external ? { target: '_blank' } : {})}
                    >
                      <AlternativeImage icon={alt.icon} source={source} />
                    </Button>
                  ) : (
                    <Button
                      className={alternativeClass}
                      variant="iconOnly"
                      aria-label={alt.label}
                      onClick={alt.onClick}
                    >
                      <AlternativeImage icon={alt.icon} source={source} />
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

type StoreBadgeProps = {
  appearance?: DownloadAppearance
  badge: 'app_store' | 'google_play'
  label: string
  href: string
  onClick: Click
}

/** The official store badge on the primary action, as a new-tab link; the image is decorative, the
    link is named. */
function StoreBadge({ badge, label, href, onClick, appearance = 'default' }: StoreBadgeProps) {
  return (
    <Button
      className={[styles.store, appearance === 'source' && styles.sourceStore]
        .filter(Boolean)
        .join(' ')}
      aria-label={label}
      href={href}
      target="_blank"
      onClick={onClick}
    >
      <img
        className={appearance === 'source' ? styles.sourceBadge : styles.badge}
        src={badge === 'app_store' ? appStoreBadge : googlePlayBadge}
        alt=""
      />
    </Button>
  )
}

export { DownloadActions, StoreBadge }
export type { DownloadActionsProps, DownloadAlternative, StoreBadgeProps }
