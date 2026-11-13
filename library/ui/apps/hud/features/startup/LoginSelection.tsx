// "Log in or Sign up": an email code (recommended), Google or MetaMask, and more methods behind MORE
// OPTIONS. The email gets its code here and moves on to VerificationOtp; any other method leaves for the
// sign-in page (web) or opens it in the browser (desktop), which shows a spinner and Cancel here
// meanwhile (`inProgress`).
//
// Laid out on the startup screens' canvas, its 1080 rows centred vertically.
import { useEffect, useRef, useState } from 'react'
import { Button, MaskIcon, Spinner, SubmitField } from '../../design'
import { EMAIL_MAX, EMAIL_PATTERN } from '@explorer-core/features/auth/email'
import { emailLoginErrorCode } from '@explorer-core/features/auth/thirdweb'
import type { LoginMethod } from '@explorer-core/features/auth/sso'
import type { LoginFlow } from '../session/useEngineSession'
import { StartupBack } from './StartupBack'
import { showConnectionError } from './connectionError'
import logo from '../../assets/loading/logo-icon.webp'
import avatars from '../../assets/startup/avatars.webp'
import envelope from '../../assets/startup/envelope.png'
import warning from '../../assets/startup/warning.png'
import arrowDown from '../../assets/startup/arrow-down.png'
import google from '../../assets/startup/google.png'
import metamask from '../../assets/startup/metamask.png'
import discord from '../../assets/startup/discord.png'
import apple from '../../assets/startup/apple.png'
import x from '../../assets/startup/x.png'
import coinbase from '../../assets/startup/coinbase.png'
import walletConnect from '../../assets/startup/wallet-connect.png'
import styles from './LoginSelection.module.css'

// The More options row. Fortmatic isn't offered: the sign-in page can't sign in with it yet.
const MORE: { method: LoginMethod; label: string; icon: string }[] = [
  { method: 'discord', label: 'Discord', icon: discord },
  { method: 'apple', label: 'Apple', icon: apple },
  { method: 'x', label: 'X', icon: x },
  { method: 'coinbase', label: 'Coinbase', icon: coinbase },
  { method: 'wallet-connect', label: 'WalletConnect', icon: walletConnect }
]

export function LoginSelection({
  flow,
  initialEmail = '',
  initialEmailError = false,
  inProgress = false,
  onBack,
  onEmailSent,
  onMethod,
  onCancel
}: {
  flow: LoginFlow
  initialEmail?: string
  /** Back from the code with an address thirdweb refused. */
  initialEmailError?: boolean
  /** A browser sign-in under way (desktop): the methods give way to a spinner and Cancel. */
  inProgress?: boolean
  onBack: () => void
  onEmailSent: (email: string) => void
  onMethod: (method: LoginMethod) => void
  onCancel: () => void
}): React.JSX.Element {
  const [email, setEmail] = useState(initialEmail)
  const [invalid, setInvalid] = useState(initialEmailError)
  const [sending, setSending] = useState(false)
  const [more, setMore] = useState(false)
  // Back during a send: its answer comes to nothing
  const live = useRef(true)
  useEffect(() => {
    live.current = true
    return () => {
      live.current = false
    }
  }, [])
  const valid = EMAIL_PATTERN.test(email)

  const sendCode = (): void => {
    setSending(true)
    flow.sendEmailCode(email).then(
      () => {
        if (!live.current) return
        setSending(false)
        onEmailSent(email)
      },
      (e: unknown) => {
        if (!live.current) return
        setSending(false)
        if (emailLoginErrorCode(e) === 'invalid-email') setInvalid(true)
        else showConnectionError()
      }
    )
  }

  const locked = flow.busy || sending

  return (
    <div className={styles.frame}>
      <StartupBack className={styles.back} onClick={onBack} />
      <img className={styles.logo} src={logo} alt="Decentraland" />
      <h1 className={styles.title}>Log in or Sign up</h1>

      {inProgress ? (
        <div className={styles.inProgress}>
          <Spinner size={40} variant="chase" />
          {flow.authCode != null && (
            <>
              <p className={styles.codeHint}>Verify this code matches the one in your browser:</p>
              <p className={styles.authCode} role="status" aria-label="Sign-in code">
                {flow.authCode}
              </p>
            </>
          )}
          <Button size="bare" variant="clear" className={styles.cancel} onClick={onCancel}>
            Cancel
          </Button>
        </div>
      ) : (
        <>
          {flow.emailAvailable && (
            <div className={styles.email}>
              <p className={styles.recommended}>Recommended</p>
              <SubmitField
                aria-label="Email"
                type="email"
                autoComplete="email"
                placeholder="your@email.com"
                icon={envelope}
                maxLength={EMAIL_MAX}
                value={email}
                onChange={(next) => {
                  setEmail(next)
                  if (invalid && EMAIL_PATTERN.test(next)) setInvalid(false)
                }}
                onSubmit={sendCode}
                submitLabel="NEXT"
                canSubmit={valid && !flow.busy}
                busy={sending}
                error={invalid ? 'Invalid email' : null}
                errorIcon={warning}
                autoFocus
              />
            </div>
          )}
          <div className={styles.separator}>
            <span className={styles.line} />
            or continue with
            <span className={styles.line} />
          </div>
          <Button size="modal" variant="method" className={styles.google} disabled={locked} onClick={() => onMethod('google')}>
            <img src={google} alt="" width={24} height={24} />
            Google
          </Button>
          <Button size="modal" variant="method" className={styles.metamask} disabled={locked} onClick={() => onMethod('metamask')}>
            <img src={metamask} alt="" width={24} height={24} />
            MetaMask
          </Button>
          <Button size="bare" variant="clear" className={styles.moreOptions} aria-expanded={more} onClick={() => setMore((open) => !open)}>
            MORE OPTIONS
            <MaskIcon src={arrowDown} size={11} height={6} className={more ? styles.arrowUp : styles.arrow} />
          </Button>
          {more && (
            <div className={styles.moreRow}>
              {MORE.map((m) => (
                <Button key={m.method} size="modal" variant="method" className={styles.moreButton} aria-label={m.label} title={m.label} disabled={locked} onClick={() => onMethod(m.method)}>
                  <img src={m.icon} alt="" width={24} height={24} />
                </Button>
              ))}
            </div>
          )}
        </>
      )}

      <img className={styles.avatars} src={avatars} alt="" draggable={false} />
    </div>
  )
}
