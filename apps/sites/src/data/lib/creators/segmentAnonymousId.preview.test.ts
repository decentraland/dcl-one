// @vitest-environment-options {"url":"https://anon-preview.vercel.app/"}
import { createAnonymousIdResolver } from './segmentAnonymousId.helpers'
import { testCookieScope } from './__tests__/cookie-scope'

testCookieScope(() => createAnonymousIdResolver(() => undefined), 'anon-preview.vercel.app')
