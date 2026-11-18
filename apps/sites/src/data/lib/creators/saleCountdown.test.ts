import { describe, expect, it } from 'vitest'
import { countdownTickMs, formatCountdown, saleTimeLeft } from './saleCountdown'

const MIN = 60_000
const HOUR = 60 * MIN
const DAY = 24 * HOUR

describe('when counting down to the end of a discount', () => {
  it('should measure what is left, never below zero, and forever without an end', () => {
    expect(saleTimeLeft(1_000 + HOUR, 1_000)).toBe(HOUR)
    expect(saleTimeLeft(1_000, 5_000)).toBe(0)
    expect(saleTimeLeft(undefined)).toBe(Infinity)
  })

  it('should show days and hours, then hours and minutes, then minutes and seconds', () => {
    expect(formatCountdown(2 * DAY + 4 * HOUR)).toBe('2d 4h')
    expect(formatCountdown(2 * DAY)).toBe('2d')
    expect(formatCountdown(4 * HOUR + 12 * MIN)).toBe('4h 12m')
    expect(formatCountdown(12 * MIN + 30_000)).toBe('12m 30s')
    expect(formatCountdown(45_000)).toBe('45s')
    expect(formatCountdown(0)).toBe('')
  })

  it('should show the unit marks it is given, in the viewer language', () => {
    expect(formatCountdown(2 * DAY + 4 * HOUR, { days: '天', hours: '小时', minutes: '分', seconds: '秒' })).toBe(
      '2天 4小时'
    )
  })

  it('should repaint every second only in the last hour', () => {
    expect(countdownTickMs(30 * MIN)).toBe(1000)
    expect(countdownTickMs(2 * HOUR)).toBe(60_000)
    expect(countdownTickMs(0)).toBe(0)
  })
})
