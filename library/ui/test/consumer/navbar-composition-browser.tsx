import { useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import '../../foundations/foundations.css'
import { Button, NavBar } from '@dcl/one-ui'

function AppAccountControl({ onConnect }: { onConnect: () => void }) {
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  return (
    <div
      onKeyDown={(event) => {
        if (event.key === 'Escape' && open) {
          event.preventDefault()
          setOpen(false)
          trigger.current?.focus()
        }
      }}
    >
      <Button
        ref={trigger}
        id="account-control"
        variant="secondary"
        size="sm"
        aria-expanded={open}
        onClick={() => {
          onConnect()
          setOpen((value) => !value)
        }}
      >
        Connect wallet
      </Button>
      {open ? (
        <div aria-label="Wallet actions">
          <Button href="/wallet/profile" variant="text" size="sm">
            Wallet profile
          </Button>
        </div>
      ) : null}
    </div>
  )
}

function CompositionExample() {
  const [connections, setConnections] = useState(0)
  const params = new URLSearchParams(window.location.search)
  const geometry = params.has('geometry')
  const [chipMode, setChipMode] = useState('none')
  const chipStyle = {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    height: 44,
    minWidth: 44,
    padding: '0 12px',
    boxSizing: 'border-box' as const
  }
  const geometryAction =
    chipMode === 'none' || chipMode === 'loading' ? null : chipMode === 'iap' ? (
      <span id="balance-action" style={chipStyle}>
        7.78
      </span>
    ) : (
      <a id="balance-action" href="/shop/credits" style={chipStyle}>
        {chipMode === 'zero' ? '0' : '7.78'}
      </a>
    )
  return (
    <>
      <header
        style={
          params.has('surface-proof')
            ? { position: 'fixed', top: 0, insetInline: 0, zIndex: 10 }
            : undefined
        }
      >
        <NavBar
          surface={
            new URLSearchParams(window.location.search).get('surface') === 'overlay'
              ? 'overlay'
              : 'solid'
          }
          sections={[
            { label: 'Discover', href: '/discover' },
            {
              label: 'Learn',
              items: [
                { label: 'Docs', href: '/docs' },
                { label: 'Blog', href: '#blog' }
              ]
            }
          ]}
          account={{ address: '0x1234', name: 'Ada' }}
          balances={{ credits: 8, mana: 2 }}
          balanceAction={
            geometry ? (
              geometryAction
            ) : (
              <Button id="balance-action" variant="secondary" size="sm">
                Buy credits
              </Button>
            )
          }
          accountControl={
            geometry ? (
              params.get('account') === 'custom' ? (
                <Button id="geometry-account" iconOnly size="lg" aria-label="Custom account">
                  AC
                </Button>
              ) : undefined
            ) : (
              <AppAccountControl onConnect={() => setConnections((value) => value + 1)} />
            )
          }
          mobileSecondaryNavigation={({ closeDrawer }) => (
            <nav aria-label="Profile sections">
              <Button
                id="secondary-link"
                href="/profile/photos"
                variant="text"
                size="md"
                onClick={(event) => {
                  event.preventDefault()
                  closeDrawer()
                }}
              >
                Photos
              </Button>
            </nav>
          )}
        />
      </header>
      <main
        style={
          params.has('surface-proof')
            ? {
                minHeight: 2200,
                background: 'repeating-linear-gradient(45deg,#eee 0 12px,#222 12px 24px)'
              }
            : undefined
        }
      >
        <p role="status">Connections: {connections}</p>
        {geometry
          ? ['none', 'loading', 'zero', 'positive', 'iap'].map((mode) => (
              <button key={mode} data-chip-mode={mode} onClick={() => setChipMode(mode)}>
                {mode}
              </button>
            ))
          : null}
      </main>
    </>
  )
}

createRoot(document.getElementById('root')!).render(<CompositionExample />)
