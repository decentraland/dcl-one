// The Donate panel's amount rule and display (features/donate/DonatePanel): the rule is the one the host sends by,
// so SEND TIP is never enabled for an amount it would refuse.
import { describe, expect, it } from 'vitest'
import { formatAmount, formatBalance, parseAmount } from '../features/donate/DonatePanel'

describe('the tip amount', () => {
  it('reads whole MANA and decimals, nothing else', () => {
    expect(parseAmount('166')).toBe(166)
    expect(parseAmount('10.5')).toBe(10.5)
    for (const bad of ['10.', '.5', '', '1e3', 'abc', '-12', '1-2']) expect(parseAmount(bad)).toBeNull()
  })

  it('reads at most 18 decimals, MANA’s precision', () => {
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
    expect(formatAmount(166)).toBe('166')
    expect(formatAmount(100)).toBe('100')
    expect(formatAmount(0.5)).toBe('0.5')
  })

  it('writes a huge amount in plain digits, so it reads as too much rather than as no amount', () => {
    const huge = formatAmount(2e21)
    expect(huge).toBe('2000000000000000000000')
    expect(parseAmount(huge)).toBe(2e21)
  })
})
