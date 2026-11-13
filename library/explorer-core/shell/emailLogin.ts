// Email sign-in as the web shells run it: the shell page makes the thirdweb calls and keeps the key,
// and the app gets only the public identity (src/shell/login.ts in each host).
import { createEmailIdentity } from '../features/auth/guest'
import { emailLoginComplete, emailLoginStart, guestSign, type GuestWallet } from '../features/auth/thirdweb'
import type { AuthIdentity } from '../features/auth/sso'

// The address rule the sign-in screen applies before NEXT, checked again here.
export const EMAIL_PATTERN = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/
export const EMAIL_MAX = 100
const CODE_PATTERN = /^\d{6}$/

function emailParam(params: unknown): string {
  const email = (params as { email?: unknown } | null)?.email
  if (typeof email !== 'string' || email.length > EMAIL_MAX || !EMAIL_PATTERN.test(email)) throw new Error('bad email request')
  return email
}

/** `emailStart` {email}: send the sign-in code. */
export async function startEmailSignIn(params: unknown): Promise<void> {
  await emailLoginStart(emailParam(params))
}

/** `emailLogin` {email, code}: check the code and sign an identity with the wallet it opens. */
export async function completeEmailSignIn(params: unknown): Promise<{ identity: AuthIdentity; wallet: GuestWallet }> {
  const email = emailParam(params)
  const code = (params as { code?: unknown }).code
  if (typeof code !== 'string' || !CODE_PATTERN.test(code)) throw new Error('bad email request')
  let wallet: GuestWallet | null = null
  const identity = await createEmailIdentity(
    {
      complete: async (e, c) => (wallet = await emailLoginComplete(e, c)),
      sign: (token, message) => guestSign(token, message)
    },
    email,
    code
  )
  return { identity, wallet: wallet! }
}
