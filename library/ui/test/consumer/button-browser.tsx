import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '@dcl/one-ui/foundations.css'

const icon = (
  <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
    <path d="M2 2h16v16H2z" />
  </svg>
)
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
  </main>
)
