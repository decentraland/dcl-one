import { afterAll, beforeAll } from 'vitest'

/** jsdom does not scroll elements; the Rail section scrolls its row back to the start when it mounts. */
export function stubElementScroll(): void {
  const scrollTo = HTMLElement.prototype.scrollTo
  beforeAll(() => {
    HTMLElement.prototype.scrollTo = function (): void {}
  })
  afterAll(() => {
    HTMLElement.prototype.scrollTo = scrollTo
  })
}
