import { describe, expect, it } from 'vitest';

import { PREVIEW_ORIGIN, PREVIEW_URL, resolvePreviewUrl } from './protocol';

const PROD = 'https://wearable-preview.decentraland.org';
const STG = 'https://wearable-preview.decentraland.today';
const DEV = 'https://wearable-preview.decentraland.zone';

describe('resolvePreviewUrl', () => {
  it.each([
    { search: '', host: 'localhost:8000', url: PROD },
    { search: '', host: 'decentraland.org', url: PROD },
    { search: '', host: 'decentraland.today', url: STG },
    { search: '', host: 'decentraland.zone', url: DEV },
    { search: '?env=dev', host: 'localhost:8000', url: DEV },
    { search: '?env=stg', host: 'localhost:8000', url: STG },
    { search: '?ENV=DEV', host: 'decentraland.org', url: DEV },
    { search: '?env=prod', host: 'decentraland.zone', url: PROD },
    { search: '?env=toString', host: 'decentraland.zone', url: DEV },
    { search: '?env=other', host: 'localhost:8000', url: PROD },
  ])('should give $url for the page "$host$search"', ({ search, host, url }) => {
    expect(resolvePreviewUrl({ search, host })).toBe(url);
  });

  it('should use production for the page of this test', () => {
    expect(PREVIEW_URL).toBe(PROD);
    expect(PREVIEW_ORIGIN).toBe(PROD);
  });
});
