// The sign-in screens' corner: Exit, mute the music,
// help, and the build. The desktop client has no way to quit from the page, so it has no Exit.
import { Button, Icon, MaskIcon } from '../../design'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { BUILD_VERSION, isNativeHud } from '@hud-host'
import { useSession } from '../session/SessionContext'
import { canExit, exitApplication } from '../gate/exitApplication'
import exitIcon from '../../assets/startup/exit.png'
import audioOn from '../../assets/startup/audio-on.png'
import audioOff from '../../assets/startup/audio-off.png'
import type { LoginMusic } from './useLoginMusic'
import styles from './StartupFooter.module.css'

export function StartupFooter({ music }: { music: LoginMusic }): React.JSX.Element {
  const session = useSession()
  const native = isNativeHud()
  const openHelp = (): void => {
    if (native) session.nav('support')
    else window.open(bugReportUrl(), '_blank', 'noopener')
  }
  return (
    <footer className={styles.footer}>
      <div className={styles.buttons}>
        {canExit() && (
          <Button size="bare" variant="tint" className={`${styles.button} ${styles.exit}`} onClick={exitApplication}>
            <MaskIcon src={exitIcon} size={24} />
            Exit
          </Button>
        )}
        <Button
          size="bare"
          variant="tint"
          className={styles.button}
          aria-label={music.muted ? 'Unmute music' : 'Mute music'}
          aria-pressed={music.muted}
          onClick={music.toggleMute}
        >
          <MaskIcon src={music.muted ? audioOff : audioOn} size={24} />
        </Button>
        <Button size="bare" variant="tint" className={styles.button} aria-label="Help" onClick={openHelp}>
          <Icon name="help" size={24} />
        </Button>
      </div>
      {BUILD_VERSION != null && <p className={styles.version}>{BUILD_VERSION}</p>}
    </footer>
  )
}
