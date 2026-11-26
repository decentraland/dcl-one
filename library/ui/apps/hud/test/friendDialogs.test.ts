import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { native, showConfirm } = vi.hoisted(() => ({ native: { value: false }, showConfirm: vi.fn() }))
vi.mock('@hud-host', async (original) => ({ ...(await original<object>()), isNativeHud: () => native.value }))
vi.mock('../design', async (original) => ({ ...(await original<object>()), showConfirm }))

const { reportUser } = await import('../features/friends/friendDialogs')
const { siteUrl } = await import('@hud-host')

const ME = '0x00000000000000000000000000000000000000aa'
const OTHER = '0x00000000000000000000000000000000000000bb'
const FORM = siteUrl(`/report/players?player_address=${ME}&reported_address=${OTHER}`)

beforeEach(() => {
  native.value = false
  showConfirm.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('reportUser', () => {
  it('opens the form for both accounts in a tab on the web', async () => {
    showConfirm.mockResolvedValue(true)
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const external = vi.fn(async () => true)
    await reportUser(ME, OTHER, external)
    expect(open).toHaveBeenCalledWith(FORM, '_blank', 'noopener')
    expect(external).not.toHaveBeenCalled()
  })

  it('hands the form to the engine on native, where the HUD has no tabs', async () => {
    native.value = true
    showConfirm.mockResolvedValue(true)
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const external = vi.fn(async () => true)
    await reportUser(ME, OTHER, external)
    expect(external).toHaveBeenCalledWith(FORM)
    expect(open).not.toHaveBeenCalled()
  })

  it('opens nothing when the confirmation is cancelled', async () => {
    native.value = true
    showConfirm.mockResolvedValue(false)
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const external = vi.fn(async () => true)
    await reportUser(ME, OTHER, external)
    expect(external).not.toHaveBeenCalled()
    expect(open).not.toHaveBeenCalled()
  })
})
