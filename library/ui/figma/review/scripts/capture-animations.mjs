// Called in the browser before reading geometry: two frames can still fall inside a CSS transition.
async function settleAnimations(animations, frame) {
  for (let pass = 0; pass < 10; pass++) {
    await frame()
    const active = animations().filter(
      (animation) => !['finished', 'idle', 'paused'].includes(animation.playState)
    )
    if (!active.length) return
    const finite = []
    for (const animation of active) {
      const endTime = animation.effect?.getComputedTiming().endTime
      if (endTime === Infinity) {
        animation.pause()
        animation.currentTime = 0
      } else if (Number.isFinite(endTime)) finite.push(animation)
      else throw new Error('Cannot settle preview animation: unknown duration')
    }
    await Promise.all(finite.map((animation) => animation.finished.catch(() => {})))
  }
  throw new Error('Preview animations did not settle')
}

export { settleAnimations }
