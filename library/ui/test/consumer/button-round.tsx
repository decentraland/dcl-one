import { createRoot } from 'react-dom/client'
import { Button } from '@dcl/one-ui'
import '../../foundations/foundations.css'

const rounds = [24, 28, 34] as const
const looks = ['text', 'outline', 'dark'] as const
const iconSizes = [14, 16, 20, 24] as const

/** An inline SVG drawn at 30px, so a forced icon size shows. */
function Glyph() {
  return (
    <svg width={30} height={30} viewBox="0 0 24 24" aria-hidden="true" data-glyph>
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
      {rounds.flatMap((round) =>
        looks.flatMap((look) =>
          iconSizes.map((iconSize) => (
            <Button
              key={`${round}-${look}-${iconSize}`}
              data-case={`round ${round} ${look} ${iconSize}`}
              data-round-case={round}
              data-icon-case={iconSize}
              variant={look}
              iconOnly
              round={round}
              iconSize={iconSize}
              aria-label="Icon action"
            >
              <Glyph />
            </Button>
          ))
        )
      )}
      <Button
        data-case="round link"
        data-round-case={34}
        data-icon-case={16}
        href="#target"
        variant="outline"
        iconOnly
        round={34}
        iconSize={16}
        aria-label="Icon link"
      >
        <Glyph />
      </Button>
      {iconSizes.map((iconSize) => (
        <Button
          key={`sized-${iconSize}`}
          data-sized-case={iconSize}
          variant="text"
          size="sm"
          iconOnly
          iconSize={iconSize}
          aria-label="Icon action"
        >
          <Glyph />
        </Button>
      ))}
      <Button data-unsized-case variant="text" size="sm" iconOnly aria-label="Icon action">
        <Glyph />
      </Button>
      <span id="target" />
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
