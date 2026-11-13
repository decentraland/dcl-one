// "Welcome to Decentraland!": a new account's name (letters and numbers, 1–15), the newsletter for an
// email account, and the terms. JUMP IN, once the name is valid and the terms ticked, deploys the
// account with its look and goes straight in; Back signs out of the new account.
//
// Laid out on the startup screens' canvas, its 1080 rows centred vertically.
import { useEffect, useRef, useState } from 'react'
import { Button, Checkbox, MaskIcon, Spinner } from '../../design'
import { StartupBack } from './StartupBack'
import logo from '../../assets/loading/logo-icon.webp'
import warning from '../../assets/startup/warning.png'
import jumpIn from '../../assets/startup/jump-in.png'
import styles from './NewAccountName.module.css'

const NAME_MAX = 15
// letters and digits only, as typed; up to three times the limit, so "Character limit reached" can show
const TYPED_MAX = NAME_MAX * 3
const LEGAL = {
  terms: 'https://decentraland.org/terms',
  privacy: 'https://decentraland.org/privacy'
}

export function NewAccountName({
  email,
  saving,
  onJumpIn,
  onBack,
  onLegal
}: {
  /** The address an email sign-in used: its newsletter box shows. */
  email: string | null
  saving: boolean
  onJumpIn: (name: string, newsletter: boolean) => void
  onBack: () => void
  /** Opens the terms or the privacy policy (the desktop client opens them in the browser). */
  onLegal?: (doc: 'terms' | 'privacy') => void
}): React.JSX.Element {
  const [name, setName] = useState('')
  const [touched, setTouched] = useState(false)
  const [newsletter, setNewsletter] = useState(false)
  const [terms, setTerms] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => input.current?.focus(), [])

  const valid = name.length >= 1 && name.length <= NAME_MAX
  const error =
    touched && !valid
      ? name.length === 0
        ? "Name can't be empty."
        : 'Character limit reached'
      : null
  const legal = (doc: 'terms' | 'privacy') => (e: React.MouseEvent) => {
    if (onLegal == null) return
    e.preventDefault()
    onLegal(doc)
  }

  return (
    <div className={styles.frame}>
      <StartupBack className={styles.back} onClick={onBack} />
      <img className={styles.logo} src={logo} alt="Decentraland" />
      <h1 className={styles.title}>Welcome to Decentraland!</h1>
      <label className={styles.label} htmlFor="new-account-name">
        Username
      </label>
      <div className={`${styles.field} ${error != null ? styles.invalid : ''}`}>
        <input
          id="new-account-name"
          ref={input}
          className={styles.input}
          value={name}
          placeholder="Name"
          maxLength={TYPED_MAX}
          disabled={saving}
          aria-invalid={error != null || undefined}
          onChange={(e) => {
            setName(e.target.value.replace(/[^A-Za-z0-9]/g, '').slice(0, TYPED_MAX))
            setTouched(true)
          }}
        />
      </div>
      <p className={`${styles.counter} ${error != null ? styles.counterError : ''}`}>
        {name.length}/{NAME_MAX}
      </p>
      {error != null && (
        <p className={styles.error} role="alert">
          <MaskIcon src={warning} size={15} />
          {error}
        </p>
      )}

      <div className={styles.checks}>
        {email != null && (
          <div className={styles.row}>
            <Checkbox
              variant="light"
              checked={newsletter}
              onChange={setNewsletter}
              aria-label="Subscribe to newsletter"
            />
            <span>
              Subscribe to newsletter for updates on features, events, contests, and more.
            </span>
          </div>
        )}
        <div className={styles.row}>
          <Checkbox
            variant="light"
            checked={terms}
            onChange={setTerms}
            aria-label="I agree with the Terms of Use and Privacy Policy"
          />
          <span>
            I agree with Decentraland’s{' '}
            <a
              className={styles.link}
              href={LEGAL.terms}
              target="_blank"
              rel="noopener noreferrer"
              onClick={legal('terms')}
            >
              Terms of Use
            </a>{' '}
            and{' '}
            <a
              className={styles.link}
              href={LEGAL.privacy}
              target="_blank"
              rel="noopener noreferrer"
              onClick={legal('privacy')}
            >
              Privacy Policy
            </a>
            .*
          </span>
        </div>
        <Button
          size="start"
          className={styles.jumpIn}
          disabled={!valid || !terms || saving}
          onClick={() => onJumpIn(name, newsletter && email != null)}
        >
          Jump in
          {saving ? (
            <Spinner size={30} variant="chase" />
          ) : (
            <img src={jumpIn} alt="" width={24} height={24} />
          )}
        </Button>
      </div>
    </div>
  )
}
