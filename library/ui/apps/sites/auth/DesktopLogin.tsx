import { useCallback, useEffect, useRef, useState } from 'react'

import { Button } from '../../../components/Button/Button'

type BrowserSignIn = { identity: Promise<unknown>; cancel: () => void }

export function DesktopLogin({
  beginSignIn,
  onSignedIn
}: {
  beginSignIn: () => BrowserSignIn
  onSignedIn: () => void
}) {
  const current = useRef<BrowserSignIn | null>(null)
  const [status, setStatus] = useState<'ready' | 'waiting' | 'failed'>('ready')

  const cancel = useCallback(() => {
    current.current?.cancel()
    current.current = null
  }, [])
  useEffect(() => cancel, [cancel])

  async function continueInBrowser() {
    if (current.current) return
    let request: BrowserSignIn
    try {
      request = beginSignIn()
    } catch {
      setStatus('failed')
      return
    }
    current.current = request
    setStatus('waiting')

    const identity = await request.identity.catch(() => null)
    if (current.current !== request) return
    current.current = null
    if (identity) onSignedIn()
    else setStatus('failed')
  }

  if (status === 'waiting') {
    return (
      <>
        <p role="status">Approve the sign-in request in your browser, then return here.</p>
        <Button
          variant="secondary"
          onClick={() => {
            cancel()
            setStatus('ready')
          }}
        >
          Cancel
        </Button>
      </>
    )
  }

  return (
    <>
      <Button onClick={() => void continueInBrowser()}>Continue in your browser</Button>
      <p>Use your existing Decentraland session to approve sign-in.</p>
      {status === 'failed' && <p role="alert">The browser didn’t confirm sign-in. Try again.</p>}
    </>
  )
}
