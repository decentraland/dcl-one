// React Settings — a page inside the shared MainMenuShell (consistent top bar with
// Backpack/etc). "Settings" + category sub-tabs and a two-column grid of controls
// (light Select / ruby Toggle / arrow Slider). Data + actions via the bridge relay
// of BevyApi.getSettings / setSetting.

import { Switch } from '@dcl/one-ui'
import { memo, useEffect, useId, useMemo, useState } from 'react'
import { Select, Slider, Tabs, Tooltip, showConfirm } from '../../design'
import { MainMenuShell } from '../menu/MainMenuShell'
import type { Setting } from '@explorer-core/engine/protocol'
import type { BindingsState, EngineSession, ProfileState, SettingsState } from '../session/useEngineSession'
import { KeyBindingsTab } from './KeyBindingsTab'
import { LobbyStageTab } from './LobbyStageTab'
import { LOBBY_STAGE_PRESETS } from '@explorer-core/engine/lobbyStage'
import { clearAutoTranslate, setAutoTranslateDefault } from '@explorer-core/features/chat/translation'
import { useTranslationPrefs } from '../chat/useTranslation'
import { PING_MODES, clearPingMode, setPingDefault, usePingPrefs, type PingMode } from '../chat/ping'
import { Mark } from '../../design/Glyphs'
import { CAN_ASK_BEFORE_LEAVING } from '@hud-host'
import { setAskBeforeLeaving, useAskBeforeLeaving } from './askBeforeLeaving'
import styles from './SettingsPanel.module.css'

// The Controls tab holds the engine's Controls rows and, in a card of its own under them, the key
// bindings: (action, keys[]) rows from the bindings relay, not numeric engine settings. The tab is
// there whether or not the engine has a Controls category.
const CONTROLS_TAB = 'Controls'
// The HUD's own chat settings (translation, the notification ping) join the engine's in this tab.
const CHAT_TAB = 'Chat'
const LOBBY_TAB = 'Lobby'

// A name with a space or a capital is already written for people (the engine's 'Voice Volume',
// 'VSync'); only an identifier (fps_limit) is turned into words.
function humanize(s: string): string {
  if (/[A-Z ]/.test(s)) return s
  return s.replace(/[_-]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()).replace(/\bFps\b/g, 'FPS')
}

// The reference client's tab row: Graphics, Sounds, Controls, Chat, each with a glyph. The engine's
// other categories follow in its own order.
const TAB_ORDER = ['graphics', 'audio', 'controls', 'chat']
const TAB_LABEL: Record<string, string> = { audio: 'Sounds' }
function tabLabel(category: string): string {
  return TAB_LABEL[category.toLowerCase()] ?? humanize(category)
}
function tabRank(category: string): number {
  const i = TAB_ORDER.indexOf(category.toLowerCase())
  return i < 0 ? TAB_ORDER.length : i
}

const TAB_GLYPH: Record<string, React.JSX.Element> = {
  graphics: (
    <>
      <rect x="2.5" y="4" width="15" height="10" rx="2" />
      <path d="M6.5 17h7" />
    </>
  ),
  audio: (
    <>
      <path d="M3 8v4h3l4 3.5v-11L6 8H3z" />
      <path d="M13 7.5a3.5 3.5 0 0 1 0 5M15.5 5.5a6.5 6.5 0 0 1 0 9" />
    </>
  ),
  controls: (
    <>
      <path d="M8 2.5h4v4l-2 2-2-2v-4zM8 17.5h4v-4l-2-2-2 2v4zM2.5 8v4h4l2-2-2-2h-4zM17.5 8v4h-4l-2-2 2-2h4z" />
    </>
  ),
  chat: (
    <>
      <path d="M4.5 3.5h11a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H10l-3.5 3v-3h-2a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2z" />
      <path d="M6.5 7.2h7M6.5 10h4.5" />
    </>
  ),
}
const OTHER_GLYPH = (
  <>
    <path d="M3 6h9M15.5 6H17M3 14h1.5M8 14h9" />
    <circle cx="13.7" cy="6" r="1.8" />
    <circle cx="6.3" cy="14" r="1.8" />
  </>
)
function TabGlyph({ category }: { category: string }): React.JSX.Element {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {TAB_GLYPH[category.toLowerCase()] ?? OTHER_GLYPH}
    </svg>
  )
}

// The reference client groups a tab's rows under headings. The engine sends a category and no
// group, so the group is read from the setting's name. One table per category, in the order the
// headings show: a row goes under the first heading whose `match` takes its name, and a row none
// takes goes under the heading without a `match` (the last heading if every one has a `match`), so
// no row is ever left out. A category without a table is one unheaded grid (as the reference's
// Chat tab is).
const SECTIONS: Record<string, { title: string; match?: RegExp }[]> = {
  graphics: [
    { title: 'Experimental', match: /experimental/i },
    { title: 'General', match: /graphics quality|preset/i },
    { title: 'Display', match: /resolution|fullscreen|window|v[ -]?sync|fps|frame[ -]?rate|monitor/i },
    { title: 'Post Processing', match: /msaa|anti[ -]?alias|\baa\b|hdr|bloom|outline|ssao|occlusion|depth of field|\bdof\b|fog|cel shad/i },
    { title: 'Landscape and Foliage', match: /grass|foliage|landscape|scene (load )?distance|lod|environment/i },
    { title: 'Sun', match: /\bsun\b|\bsky\b/i },
    { title: 'Scene Lighting', match: /light|ambient/i },
    { title: 'Shadows', match: /shadow|^quality$|texture/i },
    { title: 'Other' },
    { title: 'Physics', match: /jiggle|physics|bones/i }
  ],
  audio: [{ title: 'Volume' }, { title: 'Voice Chat & Streams', match: /microphone|\bmic\b|device|mute|^voice[ _]chat$/i }],
  controls: [{ title: 'Mouse' }, { title: 'Point At', match: /point|marker/i }]
}
function sections(category: string, items: Setting[]): { title: string | null; items: Setting[] }[] {
  const table = SECTIONS[category.toLowerCase()]
  if (table == null || table.length === 0) return [{ title: null, items }]
  const out = table.map(({ title }) => ({ title, items: [] as Setting[] }))
  const open = table.findIndex((g) => g.match == null)
  const rest = open >= 0 ? open : table.length - 1
  for (const s of items) {
    const name = humanize(s.name)
    const at = table.findIndex((g) => g.match?.test(name) === true)
    out[at >= 0 ? at : rest].items.push(s)
  }
  return out.filter((g) => g.items.length > 0)
}
// The Chat tab's own rows (the ping, auto-translate) need a section to sit in when the engine
// sends no chat setting of its own.
function chatSections(isChat: boolean, groups: { title: string | null; items: Setting[] }[]): typeof groups {
  return isChat && groups.length === 0 ? [{ title: null, items: [] }] : groups
}
// The reference's words for a row the engine names otherwise.
const ROW_LABEL: Record<string, string> = { Language: 'Translate Messages to' }
function rowLabel(s: Setting): string {
  return ROW_LABEL[s.name] ?? humanize(s.name)
}
// A toggle only reads right for an off/on pair; any other pair of names (Everyone / Only friends)
// needs its names on screen, so it is a select like the longer lists.
function isBinary(s: Setting): boolean {
  if (s.namedVariants.length === 2) return s.namedVariants.every((v) => /^(off|on)$/i.test(v.name))
  return s.namedVariants.length === 0 && s.maxValue - s.minValue <= 1 && s.stepSize >= 1
}
function isSlider(s: Setting): boolean {
  return !isBinary(s) && s.namedVariants.length < 2
}
// Values are f32 widened to f64 on the way over, so 19.95 arrives as 19.950000762939453. Showing
// them at f32's own precision drops the noise, and the trailing zeros with it. Display only.
function formatValue(s: Setting): string {
  // a setting that moves in fractions shows two decimals, as the reference does (1.00, 7000.00)
  if (!Number.isInteger(s.stepSize) && s.stepSize > 0) return s.value.toFixed(2)
  return String(Number(s.value.toPrecision(7)))
}
// A 0..100 level reads as a share, as the reference shows its volumes and its scales. Only that
// range: a scale that runs 0.5 to 2 is a factor, and shows as the number it is.
function isPercent(s: Setting): boolean {
  if (s.minValue !== 0 || s.maxValue !== 100) return false
  return s.category.toLowerCase() === 'audio' || /volume|scale/i.test(s.name)
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

function Control({ s, onSet, describedBy }: { s: Setting; onSet: (name: string, value: number) => void; describedBy?: string }): React.JSX.Element {
  // A negative value is the engine saying the setting is not known yet (e.g. a server-owned
  // one before the service answers): shown, but not editable. A disabled one shows its value,
  // greyed out.
  const unset = s.value < 0
  const locked = unset || s.disabled
  if (isBinary(s)) {
    return <Switch appearance="hud" checked={s.value >= 1} disabled={locked} onCheckedChange={(c) => onSet(s.name, c ? 1 : 0)} label={s.name} labelHidden aria-describedby={describedBy} />
  }
  if (s.namedVariants.length >= 2) {
    return (
      <Select
        variant="light"
        value={unset ? '' : String(s.value)}
        disabled={locked}
        options={s.namedVariants.map((v, i) => ({ value: String(i), label: v.name }))}
        onChange={(v) => onSet(s.name, Number(v))}
        aria-label={s.name}
        aria-describedby={describedBy}
      />
    )
  }
  return (
    <Slider arrows variant="pin" value={s.value} min={s.minValue} max={s.maxValue} step={s.stepSize || 1} disabled={locked} onChange={(v) => onSet(s.name, v)} aria-label={s.name} aria-describedby={describedBy} />
  )
}

// Memoised by value so a settings re-push (new array every change) only re-renders
// the one field that actually changed — keeps the menu snappy over the busy engine.
const SettingField = memo(
  function SettingField({ s, onSet }: { s: Setting; onSet: (name: string, value: number) => void }): React.JSX.Element {
    const description = hint(s)
    const reasonId = useId()
    const label = <span className={styles.label}>{rowLabel(s)}</span>
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
          {isSlider(s) && <span className={styles.value}>{formatValue(s)}{isPercent(s) ? '%' : ''}</span>}
        </div>
        <div className={styles.control}>
          <Control s={s} onSet={onSet} describedBy={s.disabled && description ? reasonId : undefined} />
          {/* a disabled control's description, for screen readers */}
          {s.disabled && description ? <span id={reasonId} className={styles.srOnly}>{description}</span> : null}
        </div>
      </div>
    )
  },
  (prev, next) => prev.s.name === next.s.name && prev.s.value === next.s.value && prev.s.disabled === next.s.disabled && prev.onSet === next.onSet
)

/** The chat settings kept by the HUD rather than the engine: translation (the language is the
 *  engine's Language setting, in the same tab) and the notification ping's default, which a
 *  conversation's own menu can override. */
function PingField(): React.JSX.Element {
  const ping = usePingPrefs()
  return (
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          <Tooltip label="The sound for a chat line landing in a conversation; each conversation's menu can choose otherwise. Sending a message sounds only on All Messages." side="bottom" multiline className={styles.hint}>
            <span className={styles.label}>Notification Ping</span>
          </Tooltip>
        </div>
        <div className={styles.control}>
          <Select
            variant="light"
            value={ping.default}
            options={PING_MODES.map((m) => ({ value: m.mode, label: m.label }))}
            onChange={(v) => setPingDefault(v as PingMode)}
            aria-label="Notification Ping"
          />
        </div>
      </div>
  )
}
function AutoTranslateField(): React.JSX.Element {
  const prefs = useTranslationPrefs()
  return (
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          <span className={styles.label}>Auto-Translate New Conversations</span>
        </div>
        <div className={styles.control}>
          <Switch appearance="hud" checked={prefs.autoDefault} onCheckedChange={setAutoTranslateDefault} label="Auto-Translate New Conversations" labelHidden />
        </div>
      </div>
  )
}

/** The browser's question before Back or a closed tab ends a session, in the Gameplay tab where the
 *  host can ask it. */
function AskBeforeLeavingField(): React.JSX.Element {
  const ask = useAskBeforeLeaving()
  return (
      <div className={styles.field}>
        <div className={styles.fieldHead}>
          <span className={styles.label}>Ask Before Leaving the Page</span>
        </div>
        <div className={styles.control}>
          <Switch appearance="hud" checked={ask} onCheckedChange={setAskBeforeLeaving} label="Ask Before Leaving the Page" labelHidden />
        </div>
      </div>
  )
}

// The full page and the lobby's compact Settings open on the tab last picked in either.
let lastTab: string | null = null

/** Forget the last tab: for tests. */
export function resetSettingsTab(): void {
  lastTab = null
}

type SettingsPanelProps = {
  settings: SettingsState
  bindings: BindingsState
  profile: ProfileState
  lobbyStage: EngineSession['lobbyStage']
} & ({ compact?: undefined; onNavigate: (page: string) => void } | { compact: true; onNavigate?: undefined })

/** `compact`: the body alone, for the lobby's section modal, which owns the close control. */
export function SettingsPanel({ settings, bindings, profile, lobbyStage, compact, onNavigate }: SettingsPanelProps): React.JSX.Element | null {
  const categories = useMemo(
    () => {
      // the engine's own spelling of a category wins over the tab the HUD adds
      const engine = [...new Set(settings.list.map((s) => s.category))]
      const own = [CONTROLS_TAB, CHAT_TAB, LOBBY_TAB].filter((t) => !engine.some((c) => c.toLowerCase() === t.toLowerCase()))
      return [...engine, ...own].sort((a, b) => tabRank(a) - tabRank(b))
    },
    [settings.list]
  )
  // The full page stays mounted while closed, so the shared value is read on each render.
  const [, rerender] = useState(0)
  const setTab = (next: string): void => {
    lastTab = next
    rerender((n) => n + 1)
  }
  const { refresh } = settings
  // The engine changes settings itself, so each compact open re-reads them.
  useEffect(() => {
    if (compact) refresh()
  }, [compact, refresh])

  if (!settings.open && !compact) return null

  // Opens on Graphics, as the reference does; without it, on the engine's first category.
  const firstTab = categories.find((c) => tabRank(c) === 0) ?? settings.list[0]?.category ?? categories[0]
  const activeTab = lastTab && categories.includes(lastTab) ? lastTab : firstTab
  const items = settings.list.filter((s) => s.category === activeTab)
  const isControls = activeTab.toLowerCase() === CONTROLS_TAB.toLowerCase()
  const isChat = activeTab.toLowerCase() === CHAT_TAB.toLowerCase()
  const asksHere = CAN_ASK_BEFORE_LEAVING && activeTab.toLowerCase() === 'gameplay'
  const resetAll = (): void => {
    items.forEach((s) => settings.set(s.name, s.default))
    if (isControls) {
      void showConfirm({
        title: 'Reset key bindings?',
        body: 'All bindings return to their defaults. This cannot be undone.',
        confirmLabel: 'Reset'
      }).then((ok) => ok && bindings.reset())
    }
    if (activeTab === LOBBY_TAB) lobbyStage.set({ preset: LOBBY_STAGE_PRESETS[0].id })
    if (asksHere) setAskBeforeLeaving(false)
    if (isChat) {
      setAutoTranslateDefault(false)
      clearAutoTranslate()
      setPingDefault('all')
      clearPingMode()
    }
  }

  const head = (
    <div className={styles.head}>
      <h1 className={styles.title}>Settings</h1>
      <Tabs
        variant="section"
        className={styles.tabs}
        items={categories.map((c) => ({ id: c, label: tabLabel(c), icon: <TabGlyph category={c} /> }))}
        value={activeTab}
        onChange={setTab}
        aria-label="Settings categories"
      />
      <button type="button" className={styles.reset} onClick={resetAll}>
        <Mark name="reset" /> Reset all defaults
      </button>
    </div>
  )
  const scroll = (
    <div className={styles.scroll}>
      {activeTab === LOBBY_TAB ? (
        <div className={styles.card} role="group" aria-label="Lobby settings">
          <LobbyStageTab preset={lobbyStage.preset} onPick={(preset) => lobbyStage.set({ preset })} />
        </div>
      ) : (
        (items.length > 0 || !isControls) && (
          <div className={styles.card} role="group" aria-label={`${tabLabel(activeTab)} settings`}>
            {items.length === 0 && !isChat ? (
              <div className={styles.empty}>No settings available.</div>
            ) : (
              chatSections(isChat, sections(activeTab, items)).map((g, i, all) => (
                <section key={g.title ?? 'all'} className={styles.section}>
                  {g.title != null && <h2 className={styles.sectionTitle}>{g.title}</h2>}
                  <div className={styles.grid}>
                    {isChat && i === 0 && <PingField />}
                    {g.items.map((s) => (
                      <SettingField key={s.name} s={s} onSet={settings.set} />
                    ))}
                    {isChat && i === all.length - 1 && <AutoTranslateField />}
                    {asksHere && i === all.length - 1 && <AskBeforeLeavingField />}
                  </div>
                </section>
              ))
            )}
          </div>
        )
      )}
      {isControls && (
        <div className={styles.card} role="group" aria-label="Key bindings">
          <KeyBindingsTab bindings={bindings} />
        </div>
      )}
    </div>
  )

  if (compact) {
    return (
      <div className={`${styles.page} ${styles.compact}`}>
        {head}
        {scroll}
      </div>
    )
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
      <div className={styles.page}>
        {head}
        {scroll}
      </div>
    </MainMenuShell>
  )
}
