// The shared Donate panel's amount rule (library/ui/apps/hud/features/donate, tested there) against what this host's
// shell sends by (manaToWei): SEND TIP is never enabled for an amount the shell would refuse.
import { describe, expect, it } from 'vitest'
import { parseAmount } from '@hud/features/donate/DonatePanel'
import { manaToWei } from '../../explorer/components/manaTip'

describe('the tip amount', () => {
  it('accepts exactly what the shell can send', () => {
    for (const text of ['166', '10.5', '0.01', '10.', '.5', '1.' + '1'.repeat(18)]) expect(parseAmount(text) != null).toBe(manaToWei(text) != null)
  })
})
