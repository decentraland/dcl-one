// The Donate panel's amount rule (features/donate/ui): the one the shell sends by, so SEND TIP is never enabled for
// an amount the shell would refuse.
import { describe, expect, it } from 'vitest'
import { formatAmount, formatBalance, parseAmount } from '../features/donate/ui/DonatePanel'
import { manaToWei } from '../../explorer/components/manaTip'

describe('the tip amount', () => {
  it('reads whole MANA and decimals, nothing else', () => {
    expect(parseAmount('166')).toBe(166)
    expect(parseAmount('10.5')).toBe(10.5)
    expect(parseAmount('-12')).toBe(12)
    for (const bad of ['10.', '.5', '', '1e3', 'abc']) expect(parseAmount(bad)).toBeNull()
  })

  it('accepts exactly what the shell can send', () => {
    for (const text of ['166', '10.5', '0.01', '10.', '.5']) expect(parseAmount(text) != null).toBe(manaToWei(text) != null)
  })

  it('reads at most 18 decimals, MANA\u2019s precision', () => {
    expect(parseAmount('1.' + '1'.repeat(18))).not.toBeNull()
    expect(parseAmount('1.' + '1'.repeat(19))).toBeNull()
  })

  it('shows the balance rounded down, so it never reads more than can be tipped', () => {
    expect(formatBalance(9.996)).toBe('9.99')
    expect(formatBalance(10)).toBe('10')
  })

  it('shows at most two decimals', () => {
    expect(formatAmount(69.7234)).toBe('69.72')
    expect(formatAmount(Number.NaN)).toBe('0')
  })
})
