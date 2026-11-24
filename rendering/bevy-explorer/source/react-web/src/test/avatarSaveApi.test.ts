import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { runInNewContext } from 'node:vm'
import { expect, it, vi } from 'vitest'
import type { SetAvatarData } from '@explorer-core/engine/generated'

it('returns the saved version and carries the committed profile on a save rejection', async () => {
  const profile = { ethAddress: '0x1234', name: 'Ada', version: 3, avatar: { wearables: [] }, description: 'Saved' }
  const currentLogin = vi.fn<() => string | null>().mockReturnValue(null)
  const op = vi.fn()
    .mockResolvedValueOnce({ Ok: 4 })
    .mockResolvedValueOnce({ Err: { message: 'deploy failed', profile } })
    .mockResolvedValueOnce({ Err: { message: 'no profile', profile: null, accountChanged: true } })
    .mockRejectedValueOnce(new Error('worker closed'))
  const module = { exports: {} as { setAvatar: (data: SetAvatarData) => Promise<number> } }
  const source = readFileSync(resolve(process.cwd(), '../crates/dcl_js/src/modules/SystemApi.js'), 'utf8')
  runInNewContext(source, { module, Error, Deno: { core: { ops: { op_set_avatar: op, op_get_current_login: currentLogin } } } })

  const patch = { profileExtras: { description: 'Edited' } }
  await expect(module.exports.setAvatar(patch)).rejects.toThrow('No player profile to edit')
  expect(op).not.toHaveBeenCalled()
  currentLogin.mockReturnValue('0x1234')
  await expect(module.exports.setAvatar(patch)).resolves.toBe(4)
  expect(op).toHaveBeenCalledWith('0x1234', patch)
  const failed = module.exports.setAvatar(patch)
  await expect(failed).rejects.toBeInstanceOf(Error)
  await expect(failed).rejects.toMatchObject({ message: 'deploy failed', profile })
  await expect(module.exports.setAvatar(patch)).rejects.toMatchObject({ message: 'no profile', profile: null, accountChanged: true })
  await expect(module.exports.setAvatar(patch)).rejects.toThrow('worker closed')
  currentLogin.mockReturnValue(null)
  await expect(module.exports.setAvatar(patch)).rejects.toThrow('No player profile to edit')
  expect(op).toHaveBeenCalledTimes(4)
})
