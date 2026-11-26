import { describe, expect, it } from 'vitest';

import en from './en.json';
import es from './es.json';
import zh from './zh.json';

// The AI sharing consent text may only promise what Assistant Insights does
// (decentraland/dcl-two, docs/adr/2026-11-17-assistant-insights.md).
const copy = (locale: typeof en) => locale.editor.ai.sharing;

describe('the AI sharing consent text', () => {
  it.each([
    ['en', en, /agreement/, /180/, /35/],
    ['es', es, /acuerdo/, /180/, /35/],
    ['zh', zh, /协议/, /180/, /35/],
  ])(
    'should tie the provider promise to the agreement and state retention and backups (%s)',
    (_, locale, agreement, retention, backups) => {
      // "Keeps no copy" is a contract term, not something the service enforces.
      expect(copy(locale as typeof en).details_analysis).toMatch(agreement);
      expect(copy(locale as typeof en).details_control).toMatch(retention);
      expect(copy(locale as typeof en).details_control).toMatch(backups);
    },
  );

  it('should say that stopping keeps what was already shared, as the service does', () => {
    expect(copy(en).details_control).toMatch(/keeps what you already shared until it expires/);
  });

  it('should list the version and OS the records carry, and ratings', () => {
    expect(copy(en).details_shared).toMatch(/ratings/);
    expect(copy(en).details_shared).toMatch(/Creator Hub version and operating system/);
  });
});
