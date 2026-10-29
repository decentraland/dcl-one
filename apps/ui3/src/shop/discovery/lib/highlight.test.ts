// from: shop@94584a5c:app/src/lib/highlight.spec.ts
import { describe, it, expect } from "vitest"
import { highlightMatches } from "./highlight"

const marked = (name: string, query: string) =>
  highlightMatches(name, query)
    .map(s => (s.match ? `[${s.text}]` : s.text))
    .join("")

describe("when highlighting what the query matched in a name", () => {
  it("should mark the start of each word a term begins, whatever the case", () => {
    expect(marked("Pirate Hat", "pir")).toBe("[Pir]ate Hat")
    expect(marked("Pirate Hat", "HAT pirate")).toBe("[Pirate] [Hat]")
  })

  it("should ignore accents on either side", () => {
    expect(marked("Máscara Épica", "mascara ep")).toBe("[Máscara] [Ép]ica")
    expect(marked("Mascara", "másc")).toBe("[Masc]ara")
  })

  it("should take the longest term that fits a word", () => {
    expect(marked("Galaxy Studio", "gal galaxy")).toBe("[Galaxy] Studio")
  })

  it("should mark nothing when no word starts with a term, or the query has none", () => {
    expect(marked("Pirate Hat", "rate")).toBe("Pirate Hat")
    expect(marked("Pirate Hat", "!!!")).toBe("Pirate Hat")
    expect(marked("Pirate Hat", "")).toBe("Pirate Hat")
  })

  it("should keep punctuation and spacing exactly as they were", () => {
    expect(marked("T-Shirt (Ice)", "t ice")).toBe("[T]-Shirt ([Ice])")
    expect(marked("  Spaced  ", "sp")).toBe("  [Sp]aced  ")
  })

  it("should keep a decomposed accent on the text it shows, and match it like the composed one", () => {
    expect(marked("Ma\u0301scara", "masc")).toBe("[Másc]ara")
    expect(marked("Máscara", "ma\u0301sc")).toBe("[Másc]ara")
  })

  it("should not pretend to expand what the server expands: a ligature stays unmarked", () => {
    // The server's unaccent turns Æ into AE, so "aether" matches "Æther" there; here it simply is not marked.
    expect(marked("Æther Cloak", "aether")).toBe("Æther Cloak")
  })

  it("should return one plain segment for an empty name", () => {
    expect(highlightMatches("", "x")).toEqual([{ text: "", match: false }])
  })
})
