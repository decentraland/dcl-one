// @vitest-environment-options {"url":"http://localhost/"}
import { createAnonymousIdResolver } from './segmentAnonymousId.helpers'
import { testCookieScope } from './__tests__/cookie-scope'

testCookieScope(() => createAnonymousIdResolver(() => undefined))
