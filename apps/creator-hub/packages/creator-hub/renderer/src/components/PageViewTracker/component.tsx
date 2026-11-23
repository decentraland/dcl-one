import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { analytics } from '#preload';

import { pageOf } from './pages';

export function PageViewTracker() {
  const { pathname } = useLocation();

  useEffect(() => {
    const view = pageOf(pathname);
    if (view) void analytics.track('Page Viewed', view);
  }, [pathname]);

  return null;
}
