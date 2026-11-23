import { useId, useState } from 'react'

import { FALLBACK_PROFILES } from '@web-core/multiplayer-test/rules'
import type { MpReplayOutcome } from '@web-core/multiplayer-test/types'

import { Button } from '../../../../components/Button/Button'
import { Modal } from '../../../../components/Modal/Modal'
import styles from './MpReplayDialog.module.css'

type MpReplayDialogProps = {
  runId: string
  profiles: string[]
  requesting?: boolean
  outcome?: MpReplayOutcome | null
  error?: string | null
  onSubmit: (args: { tier: 'a' | 'b'; profile: string; seed: number }) => void
  onClose: () => void
}

const TIERS: Array<{
  tier: 'a' | 'b'
  title: string
  copy: string
}> = [
  {
    tier: 'a',
    title: 'Replay (exact, offline)',
    copy: "Re-delivers this run's recorded frames through the CRDT oracle under the profile below. Same bundle + profile + seed produces a byte-identical outcome. Scene code is not re-executed."
  },
  {
    tier: 'b',
    title: 'Reproduce (live, same conditions)',
    copy: 'Re-runs the live harness with the same scene, seeded bot scripts, and profile. Assertions are outcome-level (converged hash, verdict thresholds) \u{2014} honestly labeled non-bit-exact.'
  }
]

export default function MpReplayDialog({
  runId,
  profiles,
  requesting = false,
  outcome = null,
  error = null,
  onSubmit,
  onClose
}: MpReplayDialogProps) {
  const formId = useId()
  const [tier, setTier] = useState<'a' | 'b'>('a')
  const [profile, setProfile] = useState('')
  const [seed, setSeed] = useState('42')
  const profileNames = profiles.length ? profiles : FALLBACK_PROFILES
  const activeProfile = profile || profileNames[0] || ''

  return (
    <Modal
      title={`Replay run ${runId}`}
      onClose={onClose}
      dismissible={!requesting}
      // A stray press outside must not discard the chosen tier, profile and seed.
      closeOnBackdrop={false}
      actions={
        outcome ? (
          <Button variant="secondary" size="lg" data-mp-action="close-replay" onClick={onClose}>
            Close
          </Button>
        ) : (
          <>
            <Button
              variant="secondary"
              size="lg"
              data-mp-action="close-replay"
              onClick={onClose}
              disabled={requesting}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="lg"
              form={formId}
              data-mp-action="replay"
              loading={requesting}
            >
              {tier === 'a' ? 'Replay offline' : 'Reproduce live'}
            </Button>
          </>
        )
      }
    >
      {outcome ? (
        <div className={styles.body} data-mp-replay-result={outcome.tier}>
          <h3 className={styles.resultTitle}>
            {outcome.tier === 'a'
              ? 'Replay (exact, offline) finished'
              : 'Reproduce (live, same conditions) started'}
          </h3>
          {outcome.tier === 'a' ? (
            <>
              <div className={styles.row}>
                <span>Converged-state hash</span>
                <code data-mp-outcome-hash="">{outcome.hash ?? '\u{2014}'}</code>
              </div>
              {outcome.decisionHash && (
                <div className={styles.row}>
                  <span>Decision-log hash</span>
                  <code>{outcome.decisionHash}</code>
                </div>
              )}
              <p className={styles.note}>
                Bit-deterministic: the same bundle, profile, and seed always yields these hashes.
              </p>
            </>
          ) : (
            <>
              {outcome.id && (
                <div className={styles.row}>
                  <span>New run</span>
                  <code data-mp-replay-run="">{outcome.id}</code>
                </div>
              )}
              <p className={styles.note}>
                Live reproduction &#x2014; compare at the outcome level, not frame-by-frame.
              </p>
            </>
          )}
        </div>
      ) : (
        <form
          id={formId}
          className={styles.body}
          onSubmit={(e) => {
            e.preventDefault()
            if (!requesting) {
              onSubmit({ tier, profile: activeProfile, seed: Number(seed) || 0 })
            }
          }}
        >
          <div className={styles.tiers} role="radiogroup" aria-label="Replay tier">
            {TIERS.map((t) => (
              <label key={t.tier} className={styles.tier} data-mp-tier={t.tier}>
                <input
                  type="radio"
                  name="mp-replay-tier"
                  value={t.tier}
                  checked={tier === t.tier}
                  onChange={() => setTier(t.tier)}
                />
                <span className={styles.tierTitle}>{t.title}</span>
                <span className={styles.tierCopy}>{t.copy}</span>
              </label>
            ))}
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span>Network profile</span>
              <select
                name="mp-replay-profile"
                value={activeProfile}
                onChange={(e) => setProfile(e.target.value)}
              >
                {profileNames.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </label>
            <label className={styles.field}>
              <span>Seed</span>
              <input
                type="number"
                name="mp-replay-seed"
                value={seed}
                onChange={(e) => setSeed(e.target.value)}
              />
            </label>
          </div>

          {error && (
            <p className={styles.error} role="alert" data-mp-error="replay">
              {error}
            </p>
          )}
        </form>
      )}
    </Modal>
  )
}
