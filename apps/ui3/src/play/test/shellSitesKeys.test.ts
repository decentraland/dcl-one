// The shell records an in-app sign-in under Sites' own storage keys, so the Shop reads the account as
// Sites does. The two apps share no code here: this keeps the keys from drifting apart unnoticed.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { SIGN_IN_KIND_KEY, THIRDWEB_SESSION_KEY } from '../shell/login'

const SITES_AUTH = join(__dirname, '../../../../sites/src/data/lib/auth')

describe("the shell's sign-in keys", () => {
  it("are Sites' keys", () => {
    expect(readFileSync(join(SITES_AUTH, 'thirdweb/session.ts'), 'utf8')).toContain(`THIRDWEB_SESSION_KEY = "${THIRDWEB_SESSION_KEY}"`)
    expect(readFileSync(join(SITES_AUTH, 'signin-kind.ts'), 'utf8')).toContain(`KEY = "${SIGN_IN_KIND_KEY}"`)
  })
})
