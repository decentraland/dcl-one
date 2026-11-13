// The screens from launch until a sign-in is submitted (startupScreen.ts picks one). The sign-in
// screens share one frame: the drifting logo pattern on the purple vignette, laid out
// on a 1920x1080 canvas scaled to fit, with the footer in its corner.
import { LogoPattern } from '../../design'
import type { LoginFlow } from '../session/useEngineSession'
import { LoadingAndLogin } from '../login/LoadingAndLogin'
import { GuestOrSignUp } from './GuestOrSignUp'
import { StartupFooter } from './StartupFooter'
import { StartupSplash } from './StartupSplash'
import { startupScreen } from './startupScreen'
import type { LoginMusic } from './useLoginMusic'
import styles from './StartupFlow.module.css'

export function StartupFlow({ flow, music }: { flow: LoginFlow; music: LoginMusic }): React.JSX.Element {
  const screen = startupScreen(flow)
  if (screen === 'splash') return <StartupSplash />
  // the screens not ported yet keep today's panel
  if (screen !== 'guestOrSignUp') return <LoadingAndLogin flow={flow} />
  return (
    <div className={styles.root}>
      <LogoPattern />
      <div className={styles.canvas}>
        <GuestOrSignUp flow={flow} />
        <StartupFooter music={music} />
      </div>
    </div>
  )
}
