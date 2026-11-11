// "Your browser is on the integrated GPU" — an ordinary dialog on the popup layer, opened once the
// player is signed in when the pre-boot probe found an Intel adapter on Windows (lib/integratedGpu.ts).
// Nothing is broken: the world runs, only slower than the machine allows, and only the player can
// move the browser to the dedicated GPU. WebGPU cannot tell whether a second GPU exists at all, so
// the copy hedges: an Intel-only machine qualifies too. One fix, the browser's own flag, since it
// works the same on every Chromium and needs no Windows settings; pasted, never linked: Chromium
// blocks web pages from opening chrome:// targets (see FlagStep).

import { openPopup, ModalShell, Button } from '../../design'
import type { GpuAdapterInfo } from '@explorer-core/lib/gpu'
import { chromiumBrowser } from '@explorer-core/lib/integratedGpu'
import { FlagStep } from './FlagStep'
import styles from './IntegratedGpuGuide.module.css'

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
        on the same GPU as the browser. Moving {browser.name} over makes the world run noticeably smoother:
      </p>
      <ol className={styles.steps}>
        <FlagStep
          hint={
            <>
              In {browser.name}, paste this into a new tab, set it to <strong>Enabled</strong> and relaunch.
            </>
          }
          target={browser.flagsUrl}
        />
      </ol>
      <p className={styles.after}>Then close {browser.name} completely and open Decentraland again.</p>
    </ModalShell>
  ))
}
