// Copy and art for the HTTP error screens, shared by the Sites StatusPage (library/ui/apps/sites/status) and by
// the static pages nginx serves when the app itself cannot answer (apps/sites/scripts/gen-error-pages.mts).

export type ErrorArt = 'lost' | 'locked' | 'wave' | 'crew'

export type StatusCopy = {
  status: number
  eyebrow: string
  title: string
  message: string
  art: ErrorArt
}

/** Each art's file in library/ui/apps/sites/assets/errors and its intrinsic size. */
export const ART_FILES: Record<ErrorArt, { file: string; width: number; height: number }> = {
  lost: { file: 'lost.webp', width: 240, height: 128 },
  locked: { file: 'locked.webp', width: 160, height: 154 },
  wave: { file: 'wave.webp', width: 324, height: 176 },
  crew: { file: 'crew.webp', width: 560, height: 358 }
}

/** The logomark's own box inside GeneralDecentraland's 24-unit frame, so a 28px mark fills its 28px box. */
export const LOGOMARK_VIEW_BOX = '2 2 20 20'

const COPY: Record<number, Omit<StatusCopy, 'status'>> = {
  400: {
    eyebrow: 'Bad request',
    title: "That request didn't quite work",
    message: 'Something in the link or form was malformed. Check it and try again.',
    art: 'wave'
  },
  401: {
    eyebrow: 'Sign in required',
    title: 'Sign in to continue',
    message: 'This page needs you to be signed in. Sign in and try again.',
    art: 'locked'
  },
  403: {
    eyebrow: 'Access denied',
    title: "You don't have access here",
    message: "Your account can't open this page. If you think it should, ask its owner for access.",
    art: 'locked'
  },
  404: {
    eyebrow: 'Page not found',
    title: 'Nothing built on this parcel',
    message:
      "We couldn't find the page you were looking for. It may have moved, or the link may be wrong.",
    art: 'lost'
  },
  405: {
    eyebrow: 'Not allowed',
    title: "That action isn't supported here",
    message: "This page can't handle that kind of request. Go back and try another way.",
    art: 'wave'
  },
  408: {
    eyebrow: 'Request timeout',
    title: 'That took too long',
    message: 'Your request timed out before it finished. Check your connection and try again.',
    art: 'wave'
  },
  410: {
    eyebrow: 'Gone',
    title: 'This page has been removed',
    message: "It used to be here, but it was taken down and won't be coming back.",
    art: 'lost'
  },
  413: {
    eyebrow: 'Too large',
    title: "That's too big to send",
    message: 'The file or form you sent is larger than we accept. Try something smaller.',
    art: 'wave'
  },
  414: {
    eyebrow: 'Link too long',
    title: 'That link is too long',
    message: 'The address is longer than we can handle. Check where it came from and try again.',
    art: 'wave'
  },
  429: {
    eyebrow: 'Too many requests',
    title: 'Slow down a little',
    message: "You've made a lot of requests in a short time. Wait a moment, then try again.",
    art: 'wave'
  },
  500: {
    eyebrow: 'Server error',
    title: 'Something broke on our side',
    message: 'An unexpected error happened while loading this page. Try again in a moment.',
    art: 'crew'
  },
  502: {
    eyebrow: 'Bad gateway',
    title: "We can't reach Decentraland right now",
    message: "A service behind this page didn't answer properly. Try again in a moment.",
    art: 'crew'
  },
  503: {
    eyebrow: 'Service unavailable',
    title: "We'll be right back",
    message: 'Decentraland is busy or down for maintenance. Try again in a few minutes.',
    art: 'crew'
  },
  504: {
    eyebrow: 'Gateway timeout',
    title: "That's taking longer than it should",
    message: "A service behind this page didn't answer in time. Try again in a moment.",
    art: 'crew'
  }
}

/** Statuses that get a dedicated static page for nginx. */
export const STATIC_ERROR_STATUSES = Object.keys(COPY).map(Number)

export function statusCopy(status: number): StatusCopy {
  const exact = COPY[status]
  if (exact) return { status, ...exact }
  if (status >= 500) {
    return {
      status,
      eyebrow: `Error ${status}`,
      title: 'Something went wrong on our side',
      message:
        "This page couldn't load because of a problem on our servers. Try again in a moment.",
      art: 'crew'
    }
  }
  return {
    status,
    eyebrow: `Error ${status}`,
    title: "We couldn't open that page",
    message: "Something about this request didn't work. Go back and try again.",
    art: 'wave'
  }
}

/** 5xx pages offer "Try again"; client errors only link home and back. */
export const isRetryable = (status: number): boolean =>
  status >= 500 || status === 408 || status === 429
