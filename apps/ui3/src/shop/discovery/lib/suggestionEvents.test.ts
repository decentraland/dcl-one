// from: shop@94584a5c:app/src/lib/suggestionEvents.spec.ts
import { describe, expect, it } from "vitest"

import { MIN_SUGGESTED_ROWS, suggestedHiddenReason } from "./suggestionEvents"

const base = { enabled: true, hasSignal: true, isLoading: false, isError: false, personalized: true, rowCount: 8 }

describe("suggestedHiddenReason", () => {
  it("lets the rail through when every condition holds", () => {
    expect(suggestedHiddenReason(base)).toBeNull()
  })

  it("reports the flag first, since a rail that never asked cannot also be unpersonalised", () => {
    expect(suggestedHiddenReason({ ...base, enabled: false, personalized: false, rowCount: 0 })).toBe("flag_off")
  })

  it("reports the missing signal before anything the server could have said", () => {
    expect(suggestedHiddenReason({ ...base, hasSignal: false, isError: true })).toBe("no_signal")
  })

  it("withholds a reason while the answer is still coming, rather than guessing one", () => {
    expect(suggestedHiddenReason({ ...base, isLoading: true, personalized: false })).toBeNull()
  })

  it("reports the error ahead of the emptiness it caused", () => {
    expect(suggestedHiddenReason({ ...base, isError: true, personalized: undefined, rowCount: 0 })).toBe("error")
  })

  it("hides a rail the server would not call personalised", () => {
    expect(suggestedHiddenReason({ ...base, personalized: false })).toBe("not_personalized")
  })

  it("hides a rail too short to be worth the space", () => {
    expect(suggestedHiddenReason({ ...base, rowCount: MIN_SUGGESTED_ROWS - 1 })).toBe("too_few")
  })

  it("keeps a rail that is exactly at the floor", () => {
    expect(suggestedHiddenReason({ ...base, rowCount: MIN_SUGGESTED_ROWS })).toBeNull()
  })
})
