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
  return (
    <>
      <NavBar
        sections={[{ label: 'Discover', href: '/discover' }]}
        account={{ address: '0x1234', name: 'Ada' }}
        balances={{ credits: 8, mana: 2 }}
        balanceAction={
          <Button id="balance-action" variant="secondary" size="sm">
            Buy credits
          </Button>
        }
        accountControl={
          <AppAccountControl onConnect={() => setConnections((value) => value + 1)} />
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
      <main>
        <p role="status">Connections: {connections}</p>
      </main>
    </>
  )
}

createRoot(document.getElementById('root')!).render(<CompositionExample />)
