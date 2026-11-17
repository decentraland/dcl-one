// When chrome://gpu has filled itself in. The page renders its headings (Graphics Feature Status,
// Dawn Info, ...) at once and shows "... loading ..." under each until the GPU process answers,
// which can take seconds on a cold start; the adapter lines (`LUID=`, Windows) come last.

/** True once chrome://gpu shows the GPU process's answer: its sections, none still loading. */
export function gpuPageFilled(text: string): boolean {
  return /Graphics Feature Status/.test(text) && /LUID=|Dawn Info/.test(text) && !/\.\.\. loading \.\.\./.test(text)
}
