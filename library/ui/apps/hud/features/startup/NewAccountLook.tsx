// "Choose a Starting Look": a new account picks one of four outfits for body type A or B, worn at once
// by its live avatar on the right. CONTINUE goes on to the name; Back signs out of the new account.
//
// Laid out on the startup screens' canvas: the header from its top, the slots and CONTINUE from its
// middle, the body type from its bottom-right corner, the avatar's stage in the column on the right.
import { useEffect, useState } from 'react'
import { Button, MaskIcon } from '../../design'
import { StartupBack } from './StartupBack'
import { PRESETS, type BodyType } from './presets'
import bodyA from '../../assets/startup/body-a.png'
import bodyB from '../../assets/startup/body-b.png'
import chevron from '../../assets/startup/chevron-down.png'
import check from '../../assets/startup/check-orange.png'
import jumpIn from '../../assets/startup/jump-in.png'
import styles from './NewAccountLook.module.css'

const BODY: Record<BodyType, { label: string; icon: string }> = {
  a: { label: 'BODY TYPE A', icon: bodyA },
  b: { label: 'BODY TYPE B', icon: bodyB }
}

export interface LookChoice {
  body: BodyType
  slot: number
}

export function NewAccountLook({
  initial = { body: 'a', slot: 0 },
  onShow,
  onContinue,
  onBack
}: {
  initial?: LookChoice
  /** The choice, for the avatar to wear (and the account to deploy). */
  onShow: (choice: LookChoice) => void
  onContinue: (choice: LookChoice) => void
  onBack: () => void
}): React.JSX.Element {
  const [choice, setChoice] = useState<LookChoice>(initial)
  const [menu, setMenu] = useState(false)
  useEffect(() => {
    onShow(choice)
  }, [choice, onShow])

  return (
    <div className={styles.screen}>
      <StartupBack className={styles.back} onClick={onBack} />
      <h1 className={styles.title}>Choose a Starting Look</h1>
      <p className={styles.description}>You can always change and customize later</p>

      <div className={styles.middle}>
        <div className={styles.slots} role="radiogroup" aria-label="Starting look">
          {PRESETS[choice.body].map((preset, slot) => (
            <button
              key={`${choice.body}${slot}`}
              type="button"
              role="radio"
              aria-checked={slot === choice.slot}
              aria-label={`Look ${slot + 1}`}
              className={`${styles.slot} ${slot === choice.slot ? styles.selected : ''}`}
              onClick={() => setChoice({ body: choice.body, slot })}
            >
              <img
                className={`${styles.picture} ${choice.body === 'a' && slot === 3 ? styles.pictureLarge : ''}`}
                src={preset.picture}
                alt=""
                draggable={false}
              />
            </button>
          ))}
        </div>
        <Button
          size="bare"
          variant="primary"
          className={styles.continue}
          onClick={() => onContinue(choice)}
        >
          CONTINUE
          <img src={jumpIn} alt="" width={33} height={33} />
        </Button>
      </div>

      <div className={styles.body}>
        {menu && (
          <div className={styles.menu} role="menu" aria-label="Body type">
            {(['a', 'b'] as const).map((body) => (
              <button
                key={body}
                type="button"
                role="menuitemradio"
                aria-checked={body === choice.body}
                className={styles.option}
                onClick={() => {
                  setMenu(false)
                  setChoice({ body, slot: choice.slot })
                }}
              >
                <MaskIcon src={BODY[body].icon} size={24} className={styles.optionIcon} />
                <span className={styles.optionLabel}>{BODY[body].label}</span>
                {body === choice.body && (
                  <img className={styles.check} src={check} alt="" width={13} height={10} />
                )}
              </button>
            ))}
          </div>
        )}
        <Button
          size="bare"
          variant="whisper"
          className={styles.bodyButton}
          aria-haspopup="menu"
          aria-expanded={menu}
          onClick={() => setMenu((open) => !open)}
        >
          <MaskIcon src={BODY[choice.body].icon} size={24} />
          <span className={styles.bodyLabel}>{BODY[choice.body].label}</span>
          <MaskIcon
            src={chevron}
            size={10}
            height={6}
            className={menu ? styles.chevronUp : undefined}
          />
        </Button>
      </div>
    </div>
  )
}
