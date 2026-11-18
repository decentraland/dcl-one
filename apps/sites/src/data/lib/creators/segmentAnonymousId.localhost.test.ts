// @vitest-environment-options {"url":"http://localhost/"}
import '@/data/lib/creators/test/setup'
import { createAnonymousIdResolver } from './segmentAnonymousId.helpers'
import { testCookieScope } from './__tests__/cookie-scope'

testCookieScope(() => createAnonymousIdResolver(() => undefined))
