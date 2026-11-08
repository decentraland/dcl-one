// Brave adds per-session noise to canvas reads ("farbling") on http pages, and the fidelity metrics are computed
// from a canvas read: the same screenshot measured in two runs gave different pixel counts. Turning farbling off
// makes Brave read the true pixels, the same values Chrome reads; Chrome ignores the unknown feature.
function browserArgs(profile) {
  return [
    '--headless=new',
    '--no-sandbox',
    '--disable-extensions',
    '--disable-background-networking',
    '--disable-features=BraveFarbling',
    '--force-color-profile=srgb',
    '--remote-debugging-port=0',
    '--user-data-dir=' + profile
  ]
}

export { browserArgs }
