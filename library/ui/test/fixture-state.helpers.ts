export async function waitForFixture<Value>(
  label: string,
  read: () => Promise<Value>,
  accepts: (value: Value) => boolean,
  nextFrame: () => Promise<unknown>,
  runtimeErrors: () => readonly string[],
  maxFrames = 120
): Promise<Value> {
  if (!Number.isSafeInteger(maxFrames) || maxFrames < 1 || maxFrames > 120)
    throw new Error('Fixture frame limit must be an integer between 1 and 120')
  const assertRuntime = () => {
    const errors = runtimeErrors()
    if (errors.length)
      throw new Error(
        `${label} raised a runtime error: ${errors
          .slice(0, 3)
          .map((error) => error.slice(0, 1000))
          .join('\n')}`
      )
  }
  for (let frame = 0; frame < maxFrames; frame++) {
    assertRuntime()
    const value = await read()
    assertRuntime()
    if (accepts(value)) return value
    if (frame + 1 < maxFrames) await nextFrame()
  }
  throw new Error(`${label} did not settle within ${maxFrames} frames`)
}
