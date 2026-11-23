import { describe, expect, it } from 'vitest';

import appSource from '../../index.tsx?raw';
import { PAGES, pageOf } from './pages';

describe('pageOf', () => {
  it('should name a page by its route', () => {
    expect(pageOf('/templates')).toEqual({ page: '/templates', tab: null });
  });

  it('should keep ids and world names out and report the tab', () => {
    expect(pageOf('/manage/world/my-world.dcl.eth/settings')).toEqual({
      page: '/manage/world/:name',
      tab: 'settings',
    });
    expect(pageOf('/analytics/0b1c2d')).toEqual({ page: '/analytics/:placeId', tab: null });
  });

  it('should tell the Learn sub-pages apart', () => {
    expect(pageOf('/learn/videos')).toEqual({ page: '/learn/videos', tab: null });
  });

  it('should report nothing for an unknown path', () => {
    expect(pageOf('/nowhere')).toBeNull();
  });

  it('should report the start page as Home, which it renders', () => {
    expect(pageOf('/')).toEqual({ page: '/home', tab: null });
    expect(pageOf('/home')).toEqual({ page: '/home', tab: null });
  });

  it('should list every route the app declares, and no other', () => {
    const routes = Array.from(appSource.matchAll(/<Route[^>]*?\spath="([^"]+)"/g), m => m[1]);
    expect(routes.length).toBeGreaterThan(0);
    expect([...PAGES].sort()).toEqual([...new Set(routes)].sort());
  });
});
