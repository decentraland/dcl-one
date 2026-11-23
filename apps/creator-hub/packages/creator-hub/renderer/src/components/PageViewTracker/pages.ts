import { matchPath } from 'react-router-dom';

// The app's routes (`renderer/src/index.tsx`), most specific first.
const PAGES = [
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

/**
 * The route a path belongs to, and its tab when the route has one. The pattern stands for the
 * page so that ids and world names in the path never reach analytics.
 */
export function pageOf(pathname: string): { page: string; tab: string | null } | null {
  for (const pattern of PAGES) {
    const match = matchPath(pattern, pathname);
    if (match) return { page: pattern.replace(/\/:tab\?$/, ''), tab: match.params.tab ?? null };
  }
  return null;
}
