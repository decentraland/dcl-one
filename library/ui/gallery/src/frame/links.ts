/**
 * Examples link to fragments or to app paths; only fragments navigate, so a canvas never leaves the gallery. The
 * gallery's own links to its pages (`data-gallery-link`) are not examples and keep working.
 */
export function keepLinksInCanvas(): void {
  window.addEventListener('click', (event) => {
    const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null
    if (!anchor || anchor.hasAttribute('data-gallery-link')) return
    if (!anchor.getAttribute('href')?.startsWith('#')) event.preventDefault()
  })
}
