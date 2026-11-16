// "Enter verification code": the six digits sent to the address. A full code is checked at once: a
// right one signs in (Success, then the lobby), a wrong or expired one is refused (Incorrect code, the
// row shakes, then clears for another try), and a right one whose wallet won't sign here goes on to the
// sign-in page. Resend Code sends a new one; Back returns to the methods, and a check still under way
// then comes to nothing.
//
// Laid out on the startup screens' canvas, its 1080 rows centred vertically.
import { useEffect, useRef, useState } from 'react'
import { Button, CodeInput, MaskIcon } from '../../design'
import { emailLoginErrorCode } from '@explorer-core/features/auth/thirdweb'
import type { LoginFlow } from '../session/useEngineSession'
import { StartupBack } from './StartupBack'
import { showConnectionError } from './connectionError'
import logo from '../../assets/loading/logo-icon.webp'
import padlock from '../../assets/startup/padlock.webp'
import successIcon from '../../assets/startup/success.png'
import incorrectIcon from '../../assets/startup/incorrect.png'
import styles from './VerificationOtp.module.css'

// how long a refused code shows before the row clears
const REFUSED_MS = 1500

type Result = 'none' | 'checking' | 'success' | 'incorrect'

export function VerificationOtp({
  flow,
  email,
  onBack,
  onInvalidEmail
}: {
  flow: LoginFlow
  email: string
  onBack: () => void
  /** thirdweb refused the address itself: back to the methods, with the address marked. */
  onInvalidEmail: () => void
}): React.JSX.Element {
  const [code, setCode] = useState('')
  const [result, setResult] = useState<Result>('none')
  const [resending, setResending] = useState(false)
  const abort = useRef(new AbortController())
  useEffect(() => {
    abort.current = new AbortController()
    return () => abort.current.abort()
  }, [])

  // a refused code shows, then clears for another try
  useEffect(() => {
    if (result !== 'incorrect') return
    const id = setTimeout(() => {
      setCode('')
      setResult('none')
    }, REFUSED_MS)
    return () => clearTimeout(id)
  }, [result])

  const check = (full: string): void => {
    setResult('checking')
    const { signal } = abort.current
    flow.verifyEmailCode(email, full, signal).then(
      () => {
        if (!signal.aborted) setResult('success')
      },
      (e: unknown) => {
        if (signal.aborted) return
        const reason = emailLoginErrorCode(e)
        if (reason === 'incorrect-code') return setResult('incorrect')
        if (reason === 'invalid-email') return onInvalidEmail()
        // the code was right but its wallet won't sign here: the sign-in page takes over
        setCode('')
        setResult('none')
        if (reason === 'sign-refused') flow.startWithAccount()
        else showConnectionError()
      }
    )
  }

  const resend = (): void => {
    setResending(true)
    flow.sendEmailCode(email).then(
      () => setResending(false),
      () => {
        setResending(false)
        showConnectionError()
      }
    )
  }

  const back = (): void => {
    abort.current.abort()
    onBack()
  }

  return (
    <div className={styles.frame}>
      {/* an accepted code is signing in: there is no going back from it */}
      {result !== 'success' && <StartupBack className={styles.back} onClick={back} />}
      <img className={styles.logo} src={logo} alt="Decentraland" />
      <h1 className={styles.title}>Enter verification code</h1>
      <p className={styles.description}>
        One time password sent to <b>{email}</b>.
        <br />
        Please enter the code below to complete verification.
      </p>
      <div className={styles.code}>
        <CodeInput
          value={code}
          onChange={setCode}
          onComplete={check}
          state={result === 'success' ? 'success' : result === 'incorrect' ? 'error' : 'normal'}
          disabled={result === 'checking' || result === 'success' || result === 'incorrect'}
          autoFocus={result === 'none'}
        />
      </div>
      {(result === 'success' || result === 'incorrect') && (
        <p className={styles.result} role="status">
          <MaskIcon src={result === 'success' ? successIcon : incorrectIcon} size={16} className={result === 'success' ? styles.successIcon : styles.incorrectIcon} />
          {result === 'success' ? 'Success' : 'Incorrect code'}
        </p>
      )}
      <Button size="bare" variant="clear" className={styles.resend} disabled={resending || result === 'checking' || result === 'success'} onClick={resend}>
        Resend Code
      </Button>
      <img className={styles.padlock} src={padlock} alt="" draggable={false} />
    </div>
  )
}
