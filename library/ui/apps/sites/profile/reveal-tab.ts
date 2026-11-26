/**
 * Scrolls a sideways-scrolling tab bar until the tab is inside it, with a gutter beside it. Only
 * the bar scrolls: the page stays where it is.
 */
export function revealTab(tab: HTMLElement | null, gutter = 16): void {
  const bar = tab?.closest<HTMLElement>('[role="tablist"]')
  if (!tab || !bar) return
  const tabBox = tab.getBoundingClientRect()
  const barBox = bar.getBoundingClientRect()
  if (tabBox.left < barBox.left + gutter) bar.scrollLeft += tabBox.left - barBox.left - gutter
  else if (tabBox.right > barBox.right - gutter)
    bar.scrollLeft += tabBox.right - barBox.right + gutter
}
