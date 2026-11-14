// Brave adds per-session noise to canvas reads ("farbling") on http pages, so a check that reads pixels would
// see different values in two runs. Turning farbling off makes Brave read the true pixels, the same values
// Chrome reads; Chrome ignores the unknown feature.
function browserArgs(profile) {
  return [
    '--headless=new',
    // Headless hosts may have no physical pointer; device emulation starts as a desktop.
    '--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4',
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
