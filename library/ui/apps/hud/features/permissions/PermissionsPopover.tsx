// Permissions popover beside the sidebar permissions button, for the scene the player stands in:
// what the scene is doing to the player now ("In force now": a held camera, a hidden avatar,
// changed movement), the player's "This visit" answers, then the stored rules for the scene, the
// realm and everywhere. An "In force now" row opens the review on the scene's rule, so what a
// default allowed without asking can be refused from here.
// A row the scene has run into this visit is highlighted — the answer or the rule in force for the
// type — so a silent refusal points at what caused it; a dismissed prompt's refusal has no row and
// shows nothing. Clicking a row clears that answer or rule at once and opens the review dialog on
// its level, so dismissing leaves it cleared. Closes on an outside click or Escape, like the
// nearby-voice popover.

import { Button, showConfirm } from '../../design'
import { useRailPopover } from '../sidebar/useRailPopover'
import type { PermissionRuleItem, PermissionRuleLevel } from '@explorer-core/engine/protocol'
import type { PermissionRules, PermissionSubject, TriggeredPermission, VisitAnswer } from '../session/useEngineSession'
import { describePermission, openPermissionReview } from './PermissionDialog'
import styles from './PermissionsPopover.module.css'

const VALUE_LABEL = { Allow: 'always', Deny: 'never', Ask: 'ask' } as const

type Level = 'visit' | 'scene' | 'realm' | 'global'

/** Which row decides `ty` for the scene: a visit answer, else the first level with a rule. */
function levelInForce(ty: string, visit: VisitAnswer[], rules: PermissionRules | null): Level | null {
  if (visit.some((v) => v.ty === ty)) return 'visit'
  if (rules == null) return null
  for (const level of ['scene', 'realm', 'global'] as const) {
    if (rules.rules[level].some((r) => r.ty === ty)) return level
  }
  return null
}

export function PermissionsPopover({
  anchor,
  visit,
  triggered,
  inForce,
  rules,
  sceneName,
  setRule,
  clearRule,
  resetDefaults,
  onClose
}: {
  /** The sidebar permissions button the popover sits beside. */
  anchor: HTMLElement | null
  visit: VisitAnswer[]
  triggered: TriggeredPermission[]
  /** The permission types acting on the player now. */
  inForce: string[]
  rules: PermissionRules | null
  sceneName: string
  setRule: (subject: PermissionSubject, allow: boolean, level: PermissionRuleLevel) => void
  clearRule: (subject: PermissionSubject) => void
  resetDefaults: () => void
  onClose: () => void
}): React.JSX.Element {
  // Centred on the button like the voice popover, then kept on screen as the list grows with the
  // scene's answers and rules.
  const { ref, style } = useRailPopover(anchor, onClose)

  const review = (subject: PermissionSubject, initial?: PermissionRuleLevel, storedOnly?: boolean): void => {
    onClose()
    openPermissionReview(subject, setRule, initial, storedOnly)
  }
  const hit = (ty: string, level: Level): boolean => triggered.some((t) => t.ty === ty) && levelInForce(ty, visit, rules) === level
  const row = (key: string, ty: string, level: Level, value: 'Allow' | 'Deny' | 'Ask', onClick: () => void): React.JSX.Element => (
    <button key={key} type="button" className={styles.row} data-hit={hit(ty, level) || undefined} onClick={onClick}>
      {describePermission(ty)}
      <span className={styles.value} data-value={value}>
        {VALUE_LABEL[value]}
      </span>
    </button>
  )
  const ruleSection = (title: string, scope: 'scene' | 'realm' | 'global', items: PermissionRuleItem[]): React.JSX.Element | null =>
    items.length === 0 || rules == null ? null : (
      <>
        <div className={styles.section}>{title}</div>
        {items.map((r) =>
          row(`${scope}|${r.ty}`, r.ty, scope, r.allow, () => {
            const subject: PermissionSubject = { ty: r.ty, sceneName, scene: rules.scene, realm: rules.realm, scope, value: r.allow }
            clearRule(subject)
            review(subject, scope)
          })
        )}
      </>
    )
  const anyRules = rules != null && rules.rules.scene.length + rules.rules.realm.length + rules.rules.global.length > 0

  return (
    <div ref={ref} className={styles.panel} role="dialog" aria-label="Scene permissions" style={style}>
      <div className={styles.title}>SCENE PERMISSIONS</div>
      <div className={styles.divider} />
      {inForce.length > 0 && (
        <>
          <div className={styles.section}>In force now</div>
          {inForce.map((ty) => (
            <button
              key={`inForce|${ty}`}
              type="button"
              className={styles.row}
              data-in-force
              disabled={rules == null}
              onClick={() => {
                if (rules == null) return
                // a rule stored for the scene may already refuse it, from the next visit on
                const value = rules.rules.scene.find((r) => r.ty === ty)?.allow ?? 'Allow'
                // a stored rule only: "This visit" would forget every scene's rule and not end it
                review({ ty, sceneName, scene: rules.scene, realm: rules.realm, scope: 'scene', value }, 'scene', true)
              }}
            >
              {describePermission(ty)}
              <span className={styles.value} data-value="Allow">
                active
              </span>
            </button>
          ))}
          <div className={styles.hint}>
            {rules == null ? 'Loading this scene’s rules…' : 'Click one to choose whether this scene may do it, from your next visit.'}
          </div>
          <div className={styles.divider} />
        </>
      )}
      {visit.length === 0 && !anyRules && inForce.length === 0 ? (
        <div className={styles.empty}>Nothing decided for this scene yet: it asks when it needs a permission.</div>
      ) : null}
      {visit.length > 0 && (
        <>
          <div className={styles.section}>This visit</div>
          {visit.map((v) =>
            row(`visit|${v.ty}`, v.ty, 'visit', v.allowed ? 'Allow' : 'Deny', () => {
              const subject: PermissionSubject = { ty: v.ty, sceneName: v.sceneName, scene: v.scene, realm: v.realm, scope: 'visit', value: v.allowed ? 'Allow' : 'Deny' }
              clearRule(subject)
              review(subject, 'visit')
            })
          )}
        </>
      )}
      {anyRules && (
        <>
          {visit.length > 0 && <div className={styles.divider} />}
          <div className={styles.title}>SAVED ANSWERS</div>
          {ruleSection('This scene', 'scene', rules.rules.scene)}
          {ruleSection('This realm', 'realm', rules.rules.realm)}
          {ruleSection('Everywhere', 'global', rules.rules.global)}
        </>
      )}
      {(visit.length > 0 || anyRules) && (
        <div className={styles.hint}>
          Click one to reset it, then choose again or close.
          {triggered.length > 0 ? ' Highlighted: used by the scene this visit.' : ''}
        </div>
      )}
      <div className={styles.divider} />
      <Button
        variant="ghost"
        size="sm"
        className={styles.reset}
        onClick={() => {
          onClose()
          void showConfirm({
            title: 'Reset permissions?',
            body: 'Every answer you have saved is forgotten, for every scene and realm. Permissions go back to the built-in defaults.',
            confirmLabel: 'Reset'
          }).then((ok) => ok && resetDefaults())
        }}
      >
        Reset defaults
      </Button>
    </div>
  )
}
