/** Examples link to fragments or to app paths; only fragments navigate, so a canvas never leaves the gallery. */
export function keepLinksInCanvas(): void {
  window.addEventListener('click', (event) => {
    const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null
    if (anchor && !anchor.getAttribute('href')?.startsWith('#')) event.preventDefault()
  })
}
