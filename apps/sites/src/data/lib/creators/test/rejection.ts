/** The error `promise` rejects with, for a test that looks at more than its message. Fails when it does not reject. */
export async function rejection(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise
  } catch (error) {
    return error
  }
  throw new Error('expected the promise to reject')
}
