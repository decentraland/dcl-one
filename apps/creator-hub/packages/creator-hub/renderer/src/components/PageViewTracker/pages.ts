import { matchPath } from 'react-router-dom';

// The app's routes (`renderer/src/index.tsx`), most specific first; `pages.spec.ts` checks that
// the two lists match.
export const PAGES = [
  '/analytics/:placeId',
  '/manage/land/:id/:tab?',
  '/manage/world/:name/:tab?',
  '/learn/videos',
  '/learn/docs',
  '/home',
  '/scenes',
  '/templates',
  '/collections',
  '/analytics',
  '/manage',
  '/earn',
  '/learn',
  '/more',
  '/editor',
  '/sign-in',
  '/',
];

// Routes that render the same page as another, reported under that one so its count isn't split.
const SAME_PAGE: Record<string, string> = { '/': '/home' };

/**
 * The route a path belongs to, and its tab when the route has one. The pattern stands for the
 * page so that ids and world names in the path never reach analytics.
 */
export function pageOf(pathname: string): { page: string; tab: string | null } | null {
  for (const pattern of PAGES) {
    const match = matchPath(pattern, pathname);
    if (match) {
      const page = pattern.replace(/\/:tab\?$/, '');
      return { page: SAME_PAGE[page] ?? page, tab: match.params.tab ?? null };
    }
  }
  return null;
}
