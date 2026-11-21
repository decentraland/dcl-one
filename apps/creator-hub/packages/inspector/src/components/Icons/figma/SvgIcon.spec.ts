import { readFileSync } from 'fs';
import { join } from 'path';
import { describe, expect, it } from 'vitest';

const stylesheet = readFileSync(join(__dirname, 'SvgIcon.module.css'), 'utf8');

const RULES = new Map(
  Array.from(stylesheet.matchAll(/:where\(\.([\w-]+)\)\s*\{([^}]*)\}/g), ([, className, body]) => [
    className,
    body.trim().replace(/\s+/g, ' '),
  ]),
);

describe('SvgIcon.module.css', () => {
  it('should give every rule zero specificity, so a consumer class or the parent wins', () => {
    expect(RULES.size).toBeGreaterThan(0);
    expect(stylesheet.split('{').length - 1).toBe(RULES.size);
  });

  it.each([
    ['SvgIcon', 'width: 1em; height: 1em; flex-shrink: 0;'],
    ['SvgIcon--fontSizeSmall', 'font-size: 16px;'],
    ['SvgIcon--fontSizeMedium', 'font-size: 24px;'],
    ['SvgIcon--fontSizeLarge', 'font-size: 35px;'],
    ['SvgIcon--fontSizeInherit', 'font-size: inherit;'],
    ['SvgIcon--tonePrimary', 'color: var(--primary-main);'],
    ['SvgIcon--toneSecondary', 'color: var(--white);'],
    ['SvgIcon--toneError', 'color: var(--error-main);'],
    ['SvgIcon--toneDisabled', 'color: var(--palette-whiteTransparent-soft);'],
  ])('should style .%s with %s', (className, declarations) => {
    expect(RULES.get(className)).toBe(declarations);
  });
});
