import { analytics, misc } from '#preload';

type LearnPage = 'learn' | 'docs' | 'videos';

/** Opens a video or doc link from the Learn pages and records which one. */
export function openLearnResource(
  page: LearnPage,
  kind: 'video' | 'doc',
  title: string,
  url: string,
) {
  void analytics.track('Learn Resource Opened', { page, kind, title, url });
  void misc.openExternal(url);
}
