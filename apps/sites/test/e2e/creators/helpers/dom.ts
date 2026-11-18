import { type ElementHandle, type Page } from 'puppeteer'

export const byTestId = (id: string) => `[data-testid="${id}"]`

/** Case-insensitive, so it matches text-transformed UI. */
export async function waitForText(page: Page, text: string, timeout = 20_000): Promise<void> {
  await page.waitForFunction(
    (t: string) => document.body.innerText.toLowerCase().includes(t.toLowerCase()),
    { timeout },
    text
  )
}

export async function waitForTestId(page: Page, id: string, timeout = 20_000): Promise<ElementHandle<Element>> {
  const handle = await page.waitForSelector(byTestId(id), { timeout, visible: true })
  if (!handle) throw new Error(`[data-testid="${id}"] never appeared`)
  return handle
}

export async function clickTestId(page: Page, id: string, timeout = 20_000): Promise<void> {
  const handle = await waitForTestId(page, id, timeout)
  // Centered first: an element at the edge of a scrolling panel can sit under a sticky footer, which would take the click.
  await handle.evaluate(el => el.scrollIntoView({ block: 'center' }))
  await handle.click()
}

/** The `data-testid` values currently on the page that start with `prefix`. */
export async function testIdsStartingWith(page: Page, prefix: string): Promise<string[]> {
  return page.$$eval(`[data-testid^="${prefix}"]`, els => els.map(el => el.getAttribute('data-testid') ?? ''))
}

/**
 * Opens a `~/components/Select`, clicking again until its listbox shows: async processing results can reflow a
 * modal between locating the trigger and clicking it, and that click then lands beside it.
 */
async function openSelect(page: Page, testId: string, timeout = 20_000): Promise<void> {
  const listbox = byTestId(`${testId}-listbox`)
  const deadline = Date.now() + timeout
  for (;;) {
    if (await page.$(listbox)) return
    await clickTestId(page, testId, Math.max(deadline - Date.now(), 1))
    try {
      await page.waitForSelector(listbox, { visible: true, timeout: 2_000 })
      return
    } catch (error) {
      if (Date.now() >= deadline) throw error
    }
  }
}

/** Opens a `~/components/Select` and picks the option with `value`. */
export async function selectOption(page: Page, testId: string, value: string): Promise<void> {
  await openSelect(page, testId)
  await clickTestId(page, `${testId}-option-${value}`)
}

/** The values a `~/components/Select` offers, read from its open listbox (closed again afterwards). */
export async function selectOptions(page: Page, testId: string): Promise<string[]> {
  await openSelect(page, testId)
  const values = await testIdsStartingWith(page, `${testId}-option-`)
  await page.keyboard.press('Escape')
  return values.map(id => id.slice(`${testId}-option-`.length))
}

/** Waits until the element is present and enabled (neither `disabled` nor `aria-disabled`). */
export async function waitForEnabled(page: Page, id: string, timeout = 20_000): Promise<void> {
  await page.waitForFunction(
    (selector: string) => {
      const el = document.querySelector(selector)
      return !!el && !el.hasAttribute('disabled') && el.getAttribute('aria-disabled') !== 'true'
    },
    { timeout },
    byTestId(id)
  )
}

/** Whether the element is on the page but not visible: false when it is missing, so a renamed test id fails. */
export async function isHidden(page: Page, id: string): Promise<boolean> {
  const handle = await page.$(byTestId(id))
  return handle ? !(await handle.isVisible()) : false
}

/** Waits for a toast of the given type, without depending on its copy. */
export async function waitForToast(page: Page, type: 'success' | 'warn' | 'error' | 'info' = 'success') {
  await page.waitForSelector(`[data-testid="toast"][data-type="${type}"]`, { timeout: 20_000 })
}

/** Waits until the item editor has loaded an item in edit mode. */
export async function waitForEditor(page: Page): Promise<void> {
  await page.waitForSelector('[data-testid="item-editor-page"][data-mode="edit"]', { timeout: 30_000 })
}

/** Whether an element with the test id is on the page and visible (not `display: none`, zero-sized or hidden). */
export async function isShown(page: Page, id: string): Promise<boolean> {
  const handle = await page.$(byTestId(id))
  return handle ? handle.isVisible() : false
}

/** Toggles a `~/components/Checkbox`, whose input is visually hidden behind its label. */
export async function check(page: Page, testId: string): Promise<void> {
  await page.waitForSelector(byTestId(testId))
  await page.$eval(byTestId(testId), el => (el as HTMLInputElement).click())
}

/** Picks files in the next file chooser the page opens after `trigger`. */
export async function uploadFiles(page: Page, trigger: () => Promise<void>, paths: string[]): Promise<void> {
  const [chooser] = await Promise.all([page.waitForFileChooser({ timeout: 10_000 }), trigger()])
  await chooser.accept(paths)
}
