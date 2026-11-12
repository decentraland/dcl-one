// The Donate panel, drawn by the web shell over the app (shell/donate/overlay): the
// player picks a suggested amount or types one, in MANA or USD, and SEND TIP hands it to the shell, which signs.
// Its steps: loading, the form, the tip being prepared (the wallet's window), processing (an outcome not yet
// known), sent (rays, OK) and failed (TRY AGAIN, Contact Support).

import { useState } from 'react'
import { Avatar, Button, Modal, ModalActions, ModalTitle, Spinner, VerifiedBadge } from '@hud/design'
import { MIN_TIP, TIP_AMOUNT } from '../../../lib/donateProtocol'
import type { DonateCreator, DonateHandlers, DonateView } from '../../../shell/donate/donation'
import styles from './DonatePanel.module.css'

const OTHER = -1

/** An amount with at most two decimals. */
export function formatAmount(n: number): string {
  if (!Number.isFinite(n)) return '0'
  return String(Math.round(n * 100) / 100)
}

/** A typed amount, by the rule the shell sends by; null when it isn't one (`10.`, `.5`, empty). */
export function parseAmount(text: string): number | null {
  const clean = text.replace(/-/g, '').trim()
  return TIP_AMOUNT.test(clean) ? Number(clean) : null
}

const shortAddress = (a: string): string => `${a.slice(0, 8)}...${a.slice(-6)}`

function Creator({ creator }: { creator: DonateCreator }): React.JSX.Element {
  return (
    <div className={styles.creator}>
      <Avatar src={creator.face} name={creator.name ?? creator.address} color={creator.nameColor} size={40} />
      <div className={styles.creatorText}>
        {creator.name && (
          <span className={styles.creatorName} style={creator.nameColor ? ({ '--name-color': creator.nameColor } as React.CSSProperties) : undefined}>
            {creator.name}
            {/* A claimed NAME is verified; any other name is the account's to set, so its #tag says whose it is. */}
            {creator.claimed ? <VerifiedBadge size={14} /> : <span className={styles.tag}>#{creator.address.slice(-4)}</span>}
          </span>
        )}
        <span className={styles.address}>{shortAddress(creator.address)}</span>
      </div>
    </div>
  )
}

function Form({ view, handlers }: { view: Extract<DonateView, { step: 'form' }>; handlers: DonateHandlers }): React.JSX.Element {
  // The amount the player last chose (after a refused send or TRY AGAIN), else the first suggestion.
  const start = view.amount ?? formatAmount(view.presets[0] ?? 0)
  const startPreset = view.amount == null ? 0 : view.presets.findIndex((v) => formatAmount(v) === view.amount)
  const [choice, setChoice] = useState(startPreset >= 0 ? startPreset : OTHER)
  const [mana, setMana] = useState(start)
  const startNumber = parseAmount(start)
  const [usd, setUsd] = useState(view.usdRate && startNumber != null ? formatAmount(startNumber * view.usdRate) : '')
  const amount = parseAmount(mana)
  const tooLittle = amount == null || amount < MIN_TIP
  const tooMuch = amount != null && amount > view.balance
  const valid = !tooLittle && !tooMuch
  const editable = choice === OTHER

  const pick = (index: number): void => {
    setChoice(index)
    if (index === OTHER) return
    const value = view.presets[index]
    setMana(formatAmount(value))
    setUsd(view.usdRate ? formatAmount(value * view.usdRate) : '')
  }
  const onMana = (text: string): void => {
    const clean = text.replace(/-/g, '')
    setMana(clean)
    const n = parseAmount(clean)
    setUsd(view.usdRate ? formatAmount((n ?? 0) * view.usdRate) : '')
  }
  const onUsd = (text: string): void => {
    const clean = text.replace(/-/g, '')
    setUsd(clean)
    const n = parseAmount(clean)
    setMana(view.usdRate ? formatAmount((n ?? 0) / view.usdRate) : '0')
  }

  return (
    <>
      <ModalTitle
        title={
          <span className={styles.title}>
            Send a Tip to <span className={styles.scene}>{view.scene}</span>&apos;s Creator
          </span>
        }
        onClose={handlers.onClose}
      />
      <div className={styles.body}>
        <Creator creator={view.creator} />
        <div className={styles.balance} data-invalid={tooMuch || undefined}>
          <span className={styles.balanceValue}>{formatAmount(view.balance)}</span>
          <span className={styles.balanceLabel}>{tooMuch ? 'Insufficient MANA' : 'Available'}</span>
          <Button variant="outline" size="sm" onClick={handlers.onBuyMana}>
            BUY MANA
          </Button>
        </div>
        <span className={styles.label}>ENTER TIP AMOUNT</span>
        <div className={styles.presets} role="radiogroup" aria-label="Tip amount">
          {view.presets.map((value, i) => (
            <button key={i} type="button" role="radio" aria-checked={choice === i} className={styles.preset} onClick={() => pick(i)}>
              {formatAmount(value)}
            </button>
          ))}
          <button type="button" role="radio" aria-checked={editable} className={styles.preset} onClick={() => pick(OTHER)}>
            OTHER
          </button>
        </div>
        <div className={styles.inputs} data-invalid={(editable && !valid) || undefined}>
          <label className={styles.field}>
            <input
              className={styles.input}
              inputMode="decimal"
              value={mana}
              readOnly={!editable}
              onFocus={() => !editable && pick(OTHER)}
              onChange={(e) => onMana(e.target.value)}
              aria-label="Tip in MANA"
            />
            <span className={styles.unit}>MANA</span>
          </label>
          {view.usdRate != null && (
            <label className={styles.field}>
              <input
                className={styles.input}
                inputMode="decimal"
                value={usd}
                readOnly={!editable}
                onFocus={() => !editable && pick(OTHER)}
                onChange={(e) => onUsd(e.target.value)}
                aria-label="Tip in USD"
              />
              <span className={styles.unit}>USD</span>
            </label>
          )}
        </div>
        {tooLittle && <span className={styles.tip}>Tips must be ≥ {MIN_TIP} MANA</span>}
        {view.error && !tooLittle && (
          <span className={styles.tip} role="alert">
            {view.error}
          </span>
        )}
      </div>
      <ModalActions>
        <Button variant="secondary" onClick={handlers.onClose}>
          Cancel
        </Button>
        <Button disabled={!valid} onClick={() => valid && handlers.onSend?.(mana.trim())}>
          SEND TIP
        </Button>
      </ModalActions>
    </>
  )
}

export function DonatePanel({ view, handlers }: { view: DonateView; handlers: DonateHandlers }): React.JSX.Element {
  const label = 'Send a Tip'
  switch (view.step) {
    case 'loading':
      return (
        <Modal ariaLabel={label} width={480}>
          <ModalTitle title="Send a Tip" onClose={handlers.onClose} />
          <div className={styles.loading}>
            <Spinner />
          </div>
        </Modal>
      )
    case 'form':
      return (
        <Modal ariaLabel={label} width={480}>
          <Form view={view} handlers={handlers} />
        </Modal>
      )
    case 'sending':
    case 'processing':
      return (
        <Modal ariaLabel={label} width={480}>
          <div className={styles.status} role="status">
            <span className={styles.title}>Preparing {view.amount} MANA Tip for</span>
            <Creator creator={view.creator} />
            <Spinner />
            <span className={styles.note}>
              {view.step === 'sending'
                ? 'A browser window should open for you to confirm this transfer.'
                : view.final
                  ? 'Your tip is still being processed. Check your wallet’s activity before tipping again.'
                  : 'Your tip is being processed. You can close this window.'}
            </span>
          </div>
          {/* Closing while sending cancels before the wallet signs; after, the tip goes on and is watched. */}
          <ModalActions>
            <Button variant="secondary" onClick={handlers.onClose}>
              CLOSE
            </Button>
          </ModalActions>
        </Modal>
      )
    case 'sent':
      // OK or a click on the backdrop closes it.
      return (
        <div className={styles.sent} onClick={handlers.onClose}>
          <span className={styles.rays} aria-hidden="true" />
          <div className={styles.sentContent} role="status" onClick={(e) => e.stopPropagation()}>
            <Avatar src={view.creator.face} name={view.creator.name ?? view.creator.address} color={view.creator.nameColor} size={96} />
            <span className={styles.title}>{view.creator.name ? 'Tip Sent to' : `Tip Sent to ${shortAddress(view.creator.address)}`}</span>
            {view.creator.name && <span className={styles.creatorName}>{view.creator.name}</span>}
            <Button onClick={handlers.onClose}>OK</Button>
          </div>
        </div>
      )
    case 'failed':
      return (
        <Modal ariaLabel="Something Went Wrong" width={420} role="alertdialog">
          <ModalTitle title="Something Went Wrong" onClose={handlers.onClose} />
          <div className={styles.body}>
            <p className={styles.note}>Your tip wasn&apos;t delivered. Please try again or contact Support.</p>
            <button type="button" className={styles.link} onClick={handlers.onSupport}>
              Contact Support
            </button>
          </div>
          <ModalActions>
            <Button variant="secondary" onClick={handlers.onClose}>
              CLOSE
            </Button>
            <Button onClick={handlers.onRetry}>TRY AGAIN</Button>
          </ModalActions>
        </Modal>
      )
  }
}
