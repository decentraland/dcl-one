import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '../../foundations/foundations.css'

const variants = [
  'primary',
  'secondary',
  'secondaryLight',
  'outline',
  'dark',
  'text',
  'outlineOnLight',
  'tertiary',
  'quaternary',
  'inverse'
] as const
const sizes = ['xl', 'lg', 'md', 'sm'] as const
const viewports = ['desktop', 'mobile'] as const

/** An inline SVG, as the icon components render one: on its own it sits on a text baseline. */
function Glyph({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" data-glyph>
      <path fill="currentColor" d="M4 4h16v16H4z" />
    </svg>
  )
}

function Fixture() {
  return (
    <main
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 8,
        padding: 16,
        background: 'var(--dcl-neutrals-shadow)'
      }}
    >
      {viewports.flatMap((viewport) =>
        sizes.flatMap((size) => [
          ...variants.map((variant) => (
            <Button
              key={`${viewport}-${size}-${variant}`}
              data-case={`child ${variant} ${size} ${viewport}`}
              variant={variant}
              size={size}
              viewport={viewport}
              iconOnly
              aria-label="Icon action"
            >
              <Glyph size={24} />
            </Button>
          )),
          ...[16, 20, 24, 30].map((glyph) => (
            <Button
              key={`${viewport}-${size}-icon-variant-${glyph}`}
              data-case={`variant iconOnly ${size} ${viewport} ${glyph}px`}
              variant="iconOnly"
              size={size}
              viewport={viewport}
              aria-label="Icon action"
            >
              <Glyph size={glyph} />
            </Button>
          )),
          ...variants.map((variant) => (
            <Button
              key={`${viewport}-${size}-${variant}-start`}
              data-case={`startIcon ${variant} ${size} ${viewport}`}
              variant={variant}
              size={size}
              viewport={viewport}
              iconOnly
              aria-label="Icon action"
              startIcon={<Glyph size={24} />}
            />
          ))
        ])
      )}
      <Button
        data-case="link primary md desktop"
        href="#target"
        size="md"
        iconOnly
        aria-label="Icon link"
      >
        <Glyph size={20} />
      </Button>
      <Button data-text size="md" variant="secondary">
        Label <Glyph size={16} />
      </Button>
      <span id="target" />
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
