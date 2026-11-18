// @vitest-environment-options {"url":"http://127.0.0.1/"}
import { createAnonymousIdResolver } from './segmentAnonymousId.helpers'
import { testCookieScope } from './__tests__/cookie-scope'

testCookieScope(() => createAnonymousIdResolver(() => undefined))
