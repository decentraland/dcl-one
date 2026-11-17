// The lobby's /goto field: a Genesis City parcel or a World's name, checked before the pick.

import { useEffect, useId, useRef, useState } from 'react'
import { GeneralMapPin, GeneralWarningError } from '../../../../foundations/icons/general'
import { parseChatCommand } from '@explorer-core/features/chat/chatCommands'
import { isCityParcel } from '@explorer-core/features/map/cityParcels'
import { checkRealm } from '@explorer-core/lib/realmCheck'
import type { Destination } from '../session/useEngineSession'
import styles from './LobbyGoto.module.css'

export type GotoTarget = Exclude<Destination, null>

/** Where the typed text leads, as chat's /goto reads it; null when no such parcel or World exists. */
export async function gotoTarget(text: string): Promise<GotoTarget | null> {
  const cmd = parseChatCommand(`/goto ${text.trim().replace(/^\/goto\s+/i, '')}`)
  if (cmd.kind === 'genesis') return { kind: 'parcel', x: 0, y: 0 }
  if (cmd.kind === 'goto') return isCityParcel(cmd) ? { kind: 'parcel', x: cmd.x, y: cmd.y } : null
  if (cmd.kind !== 'world') return null
  // only a World's own name has a server to ask; the session checks any other realm as it travels
  if (cmd.realm.endsWith('.dcl.eth') && (await checkRealm(cmd.realm)) === 'not-found') return null
  return { kind: 'world', realm: cmd.realm }
}

/** The button's arrow, as on Jump In. */
function Arrow(): React.JSX.Element {
  return (
    <svg viewBox="0 0 26 26" width="26" height="26" aria-hidden="true">
      <path
        fill="currentColor"
        d="M19.218 12.065 14.541 7.39c-.834-.833-2.237-.244-2.237.935V9.87H7.766c-.753 0-1.363.59-1.363 1.341v3.557c0 .752.61 1.362 1.363 1.362h4.518v1.545c0 1.179 1.423 1.768 2.237.935l4.677-4.675a1.32 1.32 0 0 0 .02-1.87"
      />
    </svg>
  )
}

export function LobbyGoto({ disabled, onPick }: { /** No pick yet: sign-in is still running. */ disabled: boolean; onPick: (target: GotoTarget) => void }): React.JSX.Element {
  const id = useId()
  const inputRef = useRef<HTMLInputElement>(null)
  const [text, setText] = useState('')
  const [missing, setMissing] = useState(false)
  const [checking, setChecking] = useState(false)
  // the check under way: an edit or leaving the lobby drops its answer
  const check = useRef(0)
  useEffect(
    () => () => {
      check.current++
    },
    []
  )
  const empty = text.trim() === ''

  const submit = (e: React.FormEvent): void => {
    e.preventDefault()
    if (empty || disabled || checking) return
    const mine = ++check.current
    setChecking(true)
    void gotoTarget(text).then((target) => {
      if (check.current !== mine) return
      setChecking(false)
      if (target == null) setMissing(true)
      else onPick(target)
    })
  }

  return (
    <form className={styles.form} noValidate onSubmit={submit}>
      <label className={styles.label} htmlFor={id}>
        Jump to any Genesis City coordinate or a World
      </label>
      <div
        className={`${styles.pill} ${missing ? styles.missing : ''} ${empty ? '' : styles.filled}`.replace(/\s+/g, ' ').trim()}
        onMouseDown={(e) => {
          // a press on the pill's own surface is a press on its field
          if ((e.target as HTMLElement).closest('input, button') != null) return
          e.preventDefault()
          inputRef.current?.focus()
        }}
      >
        <GeneralMapPin className={styles.pin} size={18} aria-hidden="true" />
        <span className={styles.prefix} aria-hidden="true">
          /goto
        </span>
        <input
          ref={inputRef}
          id={id}
          className={styles.input}
          type="text"
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          placeholder="e.g. 3,-5 or mana.dcl.eth"
          aria-invalid={missing || undefined}
          aria-describedby={missing ? `${id}-missing` : undefined}
          value={text}
          onChange={(e) => {
            check.current++
            setChecking(false)
            setMissing(false)
            setText(e.target.value)
          }}
        />
        <button type="submit" className={styles.go} aria-label="Go to destination" disabled={empty || disabled || checking}>
          <Arrow />
        </button>
      </div>
      {missing && (
        <p id={`${id}-missing`} className={styles.feedback} role="alert">
          <GeneralWarningError size={15} aria-hidden="true" />
          Coordinates or World name doesn't exist
        </p>
      )}
    </form>
  )
}
