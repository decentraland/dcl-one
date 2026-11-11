import { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'

const icon = (
  <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
    <path d="M2 2h16v16H2z" />
  </svg>
)
function TransitionControls() {
  const [loading, setLoading] = useState(false)
  useEffect(() => {
    const update = (event: Event) => setLoading((event as CustomEvent<boolean>).detail)
    window.addEventListener('button-busy', update)
    return () => window.removeEventListener('button-busy', update)
  }, [])
  return (
    <>
      <Button data-control="transition" loading={loading}>
        Save changes
      </Button>
      <Button data-control="transition-link" loading={loading} href="/save">
        Save changes
      </Button>
    </>
  )
}

createRoot(document.getElementById('root')!).render(
  <main>
    {([undefined, 'hover', 'pressed'] as const).map((state) => (
      <Button
        key={state ?? 'base'}
        data-control={state ?? 'base'}
        variant="outlineOnLight"
        size="md"
        forceState={state}
        leadingIcon={icon}
        trailingIcon={icon}
      >
        A longer button label
      </Button>
    ))}
    <Button
      data-control="icon"
      variant="iconOnly"
      size="md"
      forceState="pressed"
      aria-label="Close"
    >
      {icon}
    </Button>
    <Button
      data-control="loading"
      variant="outlineOnLight"
      size="md"
      loading
      leadingIcon={icon}
      trailingIcon={icon}
    >
      A longer button label
    </Button>
    <Button data-control="plain" variant="outlineOnLight" size="md">
      A longer button label
    </Button>
    <Button data-control="loading-plain" variant="outlineOnLight" size="md" loading>
      A longer button label
    </Button>
    <Button
      data-control="aria-disabled"
      variant="outlineOnLight"
      size="md"
      aria-disabled
      leadingIcon={icon}
      trailingIcon={icon}
    >
      A longer button label
    </Button>
    <Button
      data-control="native-disabled"
      variant="outlineOnLight"
      size="md"
      disabled
      leadingIcon={icon}
      trailingIcon={icon}
    >
      A longer button label
    </Button>
    <TransitionControls />
  </main>
)
