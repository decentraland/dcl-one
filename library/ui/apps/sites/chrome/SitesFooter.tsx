import {
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentType,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode
} from 'react'
import { Button } from '../shared/Button/Button'
import {
  GeneralDownArrow,
  SocialDiscord,
  SocialGitHub,
  SocialInstagram,
  SocialLinkedin,
  SocialTiktok,
  SocialX,
  SocialYoutube,
  type IconProps
} from '../../../foundations/icons'
import wordmark from '../assets/dcl_name.svg'
import flagCn from '../assets/flags/1f1e8-1f1f3.svg'
import flagEs from '../assets/flags/1f1ea-1f1f8.svg'
import flagFr from '../assets/flags/1f1eb-1f1f7.svg'
import flagGb from '../assets/flags/1f1ec-1f1e7.svg'
import flagJp from '../assets/flags/1f1ef-1f1f5.svg'
import flagKr from '../assets/flags/1f1f0-1f1f7.svg'
import styles from './SitesFooter.module.css'

type FooterLink = { label: ReactNode; href: string; external?: boolean }
type FooterColumn = { id: string; title: ReactNode; links: readonly FooterLink[] }
type FooterLocale = { code: string; label: string }

type SitesFooterProps = {
  /** Alt text of the Decentraland wordmark. */
  wordmarkLabel: string
  newsletter: {
    title: ReactNode
    emailLabel: string
    placeholder: string
    submitLabel: ReactNode
    sendingLabel: ReactNode
    successText: ReactNode
    invalidText: ReactNode
    errorText: ReactNode
    /** Resolves once subscribed; a rejection shows `errorText`. */
    onSubscribe: (email: string) => Promise<void>
  }
  connectLabel: ReactNode
  /** The phone layout's heading above the collapsible columns. */
  menuLabel: ReactNode
  columns: readonly FooterColumn[]
  language: {
    /** Names the list of languages. */
    listLabel: string
    /** Names the trigger with the current language, e.g. "Change language: English". */
    triggerLabel: (current: string) => string
    locales: readonly FooterLocale[]
    current: string
    onChange: (code: string) => void
  }
  legal: readonly FooterLink[]
  copyright: ReactNode
  /** A Getting started or Resources link was followed, e.g. for analytics. */
  onLinkClick?: (link: FooterLink) => void
  /** A social profile link was followed, named by its platform. */
  onSocialClick?: (platform: string) => void
}

// The flags source Sites shows for its six languages (twemoji, vendored under assets/flags).
const FLAGS: Record<string, string> = {
  en: flagGb,
  es: flagEs,
  fr: flagFr,
  zh: flagCn,
  ko: flagKr,
  ja: flagJp
}

const SOCIALS: readonly { name: string; href: string; Icon: ComponentType<IconProps> }[] = [
  { name: 'Discord', href: 'https://dcl.gg/discord', Icon: SocialDiscord },
  { name: 'GitHub', href: 'https://github.com/decentraland', Icon: SocialGitHub },
  { name: 'X', href: 'https://x.com/decentraland', Icon: SocialX },
  {
    name: 'Instagram',
    href: 'https://instagram.com/decentraland_foundation/',
    Icon: SocialInstagram
  },
  { name: 'YouTube', href: 'https://youtube.com/@decentraland_foundation', Icon: SocialYoutube },
  { name: 'TikTok', href: 'https://tiktok.com/@decentraland_fdn', Icon: SocialTiktok },
  { name: 'LinkedIn', href: 'https://linkedin.com/company/decentralandorg', Icon: SocialLinkedin }
]

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

type NewsletterState = 'idle' | 'sending' | 'ok' | 'invalid' | 'error'

function externalProps(link: FooterLink) {
  return link.external ? { target: '_blank', rel: 'noopener noreferrer' } : {}
}

function Socials({ onSocialClick }: { onSocialClick: SitesFooterProps['onSocialClick'] }) {
  return (
    <ul className={styles.socials}>
      {SOCIALS.map(({ name, href, Icon }) => (
        <li key={name}>
          <a
            className={styles.social}
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={name}
            onClick={onSocialClick && (() => onSocialClick(name))}
          >
            <Icon size={24} aria-hidden="true" />
          </a>
        </li>
      ))}
    </ul>
  )
}

function Newsletter({ newsletter }: { newsletter: SitesFooterProps['newsletter'] }) {
  const [email, setEmail] = useState('')
  const [state, setState] = useState<NewsletterState>('idle')
  const noteId = useId()

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const value = email.trim()
    if (!EMAIL_RE.test(value)) return setState('invalid')
    setState('sending')
    newsletter.onSubscribe(value).then(
      () => {
        setState('ok')
        setEmail('')
      },
      () => setState('error')
    )
  }

  const failed = state === 'invalid' || state === 'error'
  return (
    <div className={styles.newsletter}>
      <p className={styles.newsletterTitle}>{newsletter.title}</p>
      <form className={styles.newsletterForm} onSubmit={submit} noValidate>
        <input
          className={styles.email}
          type="email"
          autoComplete="email"
          aria-label={newsletter.emailLabel}
          aria-invalid={failed || undefined}
          aria-describedby={state === 'idle' || state === 'sending' ? undefined : noteId}
          placeholder={newsletter.placeholder}
          value={email}
          disabled={state === 'sending'}
          onChange={(event) => {
            setEmail(event.target.value)
            if (state !== 'idle') setState('idle')
          }}
        />
        <Button type="submit" size="sm" disabled={state === 'sending'}>
          {state === 'sending' ? newsletter.sendingLabel : newsletter.submitLabel}
        </Button>
      </form>
      {state === 'ok' ? (
        <p id={noteId} className={styles.note} role="status">
          {newsletter.successText}
        </p>
      ) : failed ? (
        <p id={noteId} className={`${styles.note} ${styles.noteError}`} role="alert">
          {state === 'invalid' ? newsletter.invalidText : newsletter.errorText}
        </p>
      ) : null}
    </div>
  )
}

/** Listbox switcher: opening focuses the selected option, arrows/Home/End move (one tab stop),
    Enter/Space pick, Escape or picking closes and returns focus to the trigger, Tab closes. */
function LanguageSwitcher({ language }: { language: SitesFooterProps['language'] }) {
  const { locales, current, onChange } = language
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const root = useRef<HTMLDivElement | null>(null)
  const trigger = useRef<HTMLButtonElement | null>(null)
  const options = useRef<(HTMLButtonElement | null)[]>([])
  const selected = locales.find((locale) => locale.code === current) ?? {
    code: current,
    label: current
  }

  const close = (restoreFocus: boolean) => {
    setOpen(false)
    if (restoreFocus) trigger.current?.focus()
  }

  const openList = () => {
    setActive(
      Math.max(
        0,
        locales.findIndex((locale) => locale.code === current)
      )
    )
    setOpen(true)
  }

  useEffect(() => {
    if (open) options.current[active]?.focus()
  }, [open, active])

  useEffect(() => {
    if (!open) return
    const onDown = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false)
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') close(true)
    }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  const onListKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const last = locales.length - 1
    const move: Record<string, number> = {
      ArrowDown: active >= last ? 0 : active + 1,
      ArrowUp: active <= 0 ? last : active - 1,
      Home: 0,
      End: last
    }
    if (event.key in move) {
      event.preventDefault()
      setActive(move[event.key]!)
    } else if (event.key === 'Tab') {
      close(false)
    }
  }

  return (
    <div className={styles.language} ref={root}>
      <button
        ref={trigger}
        type="button"
        className={styles.languageTrigger}
        aria-label={language.triggerLabel(selected.label)}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => (open ? close(false) : openList())}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            openList()
          }
        }}
      >
        {FLAGS[selected.code] ? (
          <img className={styles.flag} src={FLAGS[selected.code]} alt="" width={16} height={16} />
        ) : null}
        {selected.label}
        <GeneralDownArrow
          size={16}
          aria-hidden="true"
          className={open ? styles.chevronOpen : styles.chevron}
        />
      </button>
      {open ? (
        <div
          className={styles.languageList}
          role="listbox"
          aria-label={language.listLabel}
          onKeyDown={onListKey}
        >
          {locales.map((locale, index) => (
            <button
              key={locale.code}
              ref={(element) => {
                options.current[index] = element
              }}
              type="button"
              role="option"
              tabIndex={index === active ? 0 : -1}
              aria-selected={locale.code === current}
              className={styles.languageOption}
              onClick={() => {
                close(true)
                if (locale.code !== current) onChange(locale.code)
              }}
            >
              {FLAGS[locale.code] ? (
                <img
                  className={styles.flag}
                  src={FLAGS[locale.code]}
                  alt=""
                  width={16}
                  height={16}
                />
              ) : null}
              {locale.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}

/** The footer of the Sites home and download pages, as source Sites' LandingFooter. */
function SitesFooter({
  wordmarkLabel,
  newsletter,
  connectLabel,
  menuLabel,
  columns,
  language,
  legal,
  copyright,
  onLinkClick,
  onSocialClick
}: SitesFooterProps) {
  const [openColumn, setOpenColumn] = useState<string | null>(null)
  const panelId = useId()

  return (
    <footer className={styles.root}>
      <div className={styles.main}>
        <div className={styles.lead}>
          <img
            className={styles.wordmark}
            src={wordmark}
            alt={wordmarkLabel}
            width={384}
            height={48}
          />
          <Newsletter newsletter={newsletter} />
          <div className={`${styles.connect} ${styles.desktopOnly}`}>
            <p className={styles.sectionLabel}>{connectLabel}</p>
            <Socials onSocialClick={onSocialClick} />
          </div>
        </div>

        <nav className={styles.columns}>
          {columns.map((column) => (
            <div key={column.id} className={styles.column}>
              <p className={styles.sectionLabel}>{column.title}</p>
              <ul className={styles.links}>
                {column.links.map((link) => (
                  <li key={link.href}>
                    <a
                      className={styles.link}
                      href={link.href}
                      {...externalProps(link)}
                      onClick={onLinkClick && (() => onLinkClick(link))}
                    >
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>

        <nav className={styles.menu}>
          <p className={styles.menuLabel}>{menuLabel}</p>
          {columns.map((column) => {
            const open = openColumn === column.id
            const id = `${panelId}-${column.id}`
            return (
              <div key={column.id}>
                <button
                  type="button"
                  className={styles.menuToggle}
                  aria-expanded={open}
                  aria-controls={id}
                  onClick={() => setOpenColumn(open ? null : column.id)}
                >
                  {column.title}
                  <GeneralDownArrow
                    size={24}
                    aria-hidden="true"
                    className={open ? styles.chevronOpen : styles.chevron}
                  />
                </button>
                <ul id={id} className={styles.menuLinks} hidden={!open}>
                  {column.links.map((link) => (
                    <li key={link.href}>
                      <a
                        className={styles.menuLink}
                        href={link.href}
                        {...externalProps(link)}
                        onClick={onLinkClick && (() => onLinkClick(link))}
                      >
                        {link.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            )
          })}
        </nav>

        <div className={`${styles.connect} ${styles.phoneOnly}`}>
          <p className={styles.sectionLabel}>{connectLabel}</p>
          <Socials onSocialClick={onSocialClick} />
        </div>
      </div>

      <div className={styles.bar}>
        <div className={styles.barStart}>
          <LanguageSwitcher language={language} />
          <ul className={styles.legal}>
            {legal.map((link) => (
              <li key={link.href}>
                <a className={styles.legalLink} href={link.href} {...externalProps(link)}>
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </div>
        <p className={styles.copyright}>{copyright}</p>
      </div>
    </footer>
  )
}

export { SitesFooter }
export type { FooterColumn, FooterLink, FooterLocale, SitesFooterProps }
