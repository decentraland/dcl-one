import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const THEMES = join(__dirname, '../../src/themes');

// Read from disk: vitest stubs the import of a plain stylesheet.
const css = ['theme.css', 'ui3-bridge.css']
  .map(file => readFileSync(join(THEMES, file), 'utf8'))
  .join('\n');

/**
 * Adds the stylesheets of the app theme (`/@/themes`) to the test page, in the order of the app.
 * A spec that checks a themed colour uses it, so the token value comes from the real stylesheet.
 * Returns the function that removes them.
 */
export function mountThemeStyles(): () => void {
  const style = document.createElement('style');
  style.dataset.testTheme = '';
  style.textContent = css;
  document.head.append(style);
  return () => style.remove();
}
