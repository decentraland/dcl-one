// "Your browser is on the integrated GPU" — an ordinary dialog on the popup layer, opened once the
// player is signed in when the pre-boot probe found an Intel adapter on Windows (lib/integratedGpu.ts).
// Nothing is broken: the world runs, only slower than the machine allows, and only the player can
// move the browser to the dedicated GPU. WebGPU cannot tell whether a second GPU exists at all, so
// the copy hedges: an Intel-only machine qualifies too. Both fixes are pasted, never linked:
// Chromium blocks web pages from opening chrome:// and ms-settings: targets (see FlagStep).

import { openPopup, ModalShell, Button } from '../../design'
import type { GpuAdapterInfo } from '../../lib/gpu'
import { chromiumBrowser } from '../../lib/integratedGpu'
import { FlagStep } from './FlagStep'
import styles from './IntegratedGpuGuide.module.css'

/** Windows' Graphics settings page (Settings → System → Display → Graphics). */
export const WINDOWS_GRAPHICS_SETTINGS = 'ms-settings:display-advancedgraphics'

export interface IntegratedGpuGuideOptions {
  adapter: GpuAdapterInfo
  /** The player chose "Don't show again"; the owner stores it. "Later" and Escape store nothing. */
  onDontShowAgain: () => void
}

/** Open the guide; the returned handle closes it (App's effect cleanup). */
export function openIntegratedGpuGuide({ adapter, onDontShowAgain }: IntegratedGpuGuideOptions): () => void {
  const browser = chromiumBrowser()
  const title = 'Use your dedicated graphics card'
  return openPopup((close) => (
    <ModalShell
      title={title}
      ariaLabel={title}
      width={560}
      onClose={close}
      actionsEqual
      actions={
        <>
          <Button variant="ghost" onClick={close}>
            Later
          </Button>
          <Button
            variant="primary"
            onClick={() => {
              onDontShowAgain()
              close()
            }}
          >
            Don’t show again
          </Button>
        </>
      }
    >
      <p className={styles.lead}>
        Windows is running {browser.name} on your integrated graphics
        {adapter.description !== '' && (
          <>
            {' '}
            (<span className={styles.adapter}>{adapter.description}</span>)
          </>
        )}
        . If your computer also has a dedicated graphics card, Decentraland cannot pick it by itself: it renders
        on the same GPU as the browser. Moving {browser.name} over makes the world run noticeably smoother.
        Either way works:
      </p>
      <ol className={styles.steps}>
        <FlagStep
          hint={
            <>
              In Windows, open <strong>Settings → System → Display → Graphics</strong> (paste this into the
              address bar, or into Run with Win+R), add {browser.name} as a desktop app, set its options to{' '}
              <strong>High performance</strong> and save.
            </>
          }
          target={WINDOWS_GRAPHICS_SETTINGS}
        />
        <FlagStep
          hint={
            <>
              Or in {browser.name}, paste this into a new tab, set it to <strong>Enabled</strong> and relaunch.
            </>
          }
          target={browser.flagsUrl}
        />
      </ol>
      <p className={styles.after}>Then close {browser.name} completely and open Decentraland again.</p>
    </ModalShell>
  ))
}
