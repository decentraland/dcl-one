// React Settings — a page inside the shared MainMenuShell (consistent top bar with
// Backpack/etc). "Settings" + category sub-tabs and a two-column grid of controls
// (light Select / ruby Toggle / arrow Slider). Data + actions via the bridge relay
// of BevyApi.getSettings / setSetting.

import { memo, useMemo, useState } from 'react'
import { Select, Slider, Tabs, Toggle, Tooltip, showConfirm } from '../../design'
import { MainMenuShell } from '../menu/MainMenuShell'
import type { Setting } from '../../engine/protocol'
import type { BindingsState, ProfileState, SettingsState } from '../session/useEngineSession'
import { KeyBindingsTab } from './KeyBindingsTab'
import { clearAutoTranslate, setAutoTranslateDefault, useTranslationPrefs } from '../chat/translation'
import { PING_MODES, clearPingMode, setPingDefault, usePingPrefs, type PingMode } from '../chat/ping'
import styles from './SettingsPanel.module.css'

// Appended after the engine-derived categories: bindings are (action, keys[]) rows from the
// bindings relay, not numeric engine settings, so they get their own tab + body.
const KEY_BINDINGS_TAB = 'Key Bindings'
// The HUD's own chat settings (translation, the notification ping) join the engine's in this tab.
const CHAT_TAB = 'Chat'

function humanize(s: string): string {
  return s.replace(/[_-]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}
// A toggle only reads right for an off/on pair; any other pair of names (Everyone / Only friends)
// needs its names on screen, so it is a select like the longer lists. Any named list is a select,
// even of one choice (a monitor with a single fullscreen size): a slider has no names to show.
function isBinary(s: Setting): boolean {
  if (s.namedVariants.length === 2) return s.namedVariants.every((v) => /^(off|on)$/i.test(v.name))
  return s.namedVariants.length === 0 && s.maxValue - s.minValue <= 1 && s.stepSize >= 1
}
function isSlider(s: Setting): boolean {
  return !isBinary(s) && s.namedVariants.length === 0
}
// Values are f32 widened to f64 on the way over, so 19.95 arrives as 19.950000762939453. Showing
// them at f32's own precision drops the noise, and the trailing zeros with it. Display only.
function formatValue(v: number): string {
  return String(Number(v.toPrecision(7)))
}
// An enum's description is per variant (the general text plus the selected variant's line), and
// the engine sends every variant's up front: read the selected one so the hint tracks the choice
// without a refetch. The engine writes many descriptions for its own panel, which has no label of
// its own, so they open with the setting's name on a line of its own; the label shows the name here.
function hint(s: Setting): string {
  const text = (s.namedVariants[s.value]?.description ?? s.description).trim()
  const [first, ...rest] = text.split('\n')
  return first.trim().toLowerCase() === s.name.trim().toLowerCase() ? rest.join('\n').trim() : text
}

function Control({ s, onSet }: { s: Setting; onSet: (name: string, value: number) => void }): React.JSX.Element {
  // A negative value is the engine saying the setting is not known yet (e.g. a server-owned
  // one before the service answers): shown, but not editable.
  const unset = s.value < 0
  const disabled = unset || s.disabled
  if (isBinary(s)) {
    return <Toggle checked={s.value >= 1} disabled={disabled} onChange={(c) => onSet(s.name, c ? 1 : 0)} aria-label={s.name} />
  }
  if (s.namedVariants.length >= 1) {
    return (
      <Select
        variant="light"
        value={unset ? '' : String(s.value)}
        disabled={disabled}
        options={s.namedVariants.map((v, i) => ({ value: String(i), label: v.name }))}
        onChange={(v) => onSet(s.name, Number(v))}
        aria-label={s.name}
      />
    )
  }
  return (
    <Slider arrows value={s.value} min={s.minValue} max={s.maxValue} step={s.stepSize || 1} disabled={disabled} onChange={(v) => onSet(s.name, v)} aria-label={s.name} />
  )
}

// Memoised by value so a settings re-push (new array every change) only re-renders
// the one field that actually changed — keeps the menu snappy over the busy engine.
const SettingField = memo(
  function SettingField({ s, onSet }: { s: Setting; onSet: (name: string, value: number) => void }): React.JSX.Element {
    const description = hint(s)
    const label = <span className={styles.label}>{humanize(s.name)}</span>
    return (
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          {description ? (
            <Tooltip label={description} side="bottom" multiline className={styles.hint}>
              {label}
            </Tooltip>
          ) : (
            label
          )}
          {isSlider(s) && <span className={styles.value}>{formatValue(s.value)}</span>}
        </div>
        <Control s={s} onSet={onSet} />
      </div>
    )
  },
  // a setting's choices and disabled state can change at the same value
  (prev, next) =>
    prev.s.name === next.s.name &&
    prev.s.value === next.s.value &&
    prev.s.disabled === next.s.disabled &&
    prev.s.namedVariants.length === next.s.namedVariants.length &&
    prev.s.namedVariants.every((v, i) => v.name === next.s.namedVariants[i].name) &&
    prev.onSet === next.onSet
)

/** The chat settings kept by the HUD rather than the engine: translation (the language is the
 *  engine's Language setting, in the same tab) and the notification ping's default, which a
 *  conversation's own menu can override. */
function ChatFields(): React.JSX.Element {
  const prefs = useTranslationPrefs()
  const ping = usePingPrefs()
  return (
    <>
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          <span className={styles.label}>Auto-Translate New Conversations</span>
        </div>
        <Toggle checked={prefs.autoDefault} onChange={setAutoTranslateDefault} aria-label="Auto-Translate New Conversations" />
      </div>
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          <Tooltip label="The sound for a chat line landing in a conversation; each conversation's menu can choose otherwise." side="bottom" multiline className={styles.hint}>
            <span className={styles.label}>Notification Ping</span>
          </Tooltip>
        </div>
        <Select
          variant="light"
          value={ping.default}
          options={PING_MODES.map((m) => ({ value: m.mode, label: m.label }))}
          onChange={(v) => setPingDefault(v as PingMode)}
          aria-label="Notification Ping"
        />
      </div>
    </>
  )
}

export function SettingsPanel({
  settings,
  bindings,
  profile,
  onNavigate
}: {
  settings: SettingsState
  bindings: BindingsState
  profile: ProfileState
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  const categories = useMemo(
    () => [...new Set([...settings.list.map((s) => s.category), CHAT_TAB]), KEY_BINDINGS_TAB],
    [settings.list]
  )
  const [tab, setTab] = useState<string | null>(null)

  if (!settings.open) return null

  const activeTab = tab && categories.includes(tab) ? tab : categories[0]
  const items = settings.list.filter((s) => s.category === activeTab)
  const resetAll = (): void => {
    if (activeTab === KEY_BINDINGS_TAB) {
      void showConfirm({
        title: 'Reset key bindings?',
        body: 'All bindings return to their defaults. This cannot be undone.',
        confirmLabel: 'Reset'
      }).then((ok) => ok && bindings.reset())
      return
    }
    // a negative default is a choice the engine cannot name yet (no monitor read). Disabled
    // controls reset too: "all defaults" includes the frame-rate cap under VSync and the
    // fullscreen size while windowed, which take effect when re-enabled.
    items.forEach((s) => s.default >= 0 && settings.set(s.name, s.default))
    if (activeTab === CHAT_TAB) {
      setAutoTranslateDefault(false)
      clearAutoTranslate()
      setPingDefault('all')
      clearPingMode()
    }
  }

  const p = profile.data
  return (
    <MainMenuShell
      active="settings"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={settings.toggle}
    >
      <div className={styles.head}>
        <h1 className={styles.title}>Settings</h1>
        <Tabs items={categories.map((c) => ({ id: c, label: humanize(c) }))} value={activeTab} onChange={setTab} aria-label="Settings categories" />
        <button type="button" className={styles.reset} onClick={resetAll}>
          ↺ Reset all defaults
        </button>
      </div>

      <div className={styles.card}>
        {activeTab === KEY_BINDINGS_TAB ? (
          <KeyBindingsTab bindings={bindings} />
        ) : items.length === 0 && activeTab !== CHAT_TAB ? (
          <div className={styles.empty}>No settings available.</div>
        ) : (
          <div className={styles.grid}>
            {items.map((s) => (
              <SettingField key={s.name} s={s} onSet={settings.set} />
            ))}
            {activeTab === CHAT_TAB && <ChatFields />}
          </div>
        )}
      </div>
    </MainMenuShell>
  )
}
