import { z } from "zod";

/**
 * A form's draft kept in this tab across the sign-in page, which leaves the app: saved for the URL sign-in
 * returns to, read back once there, and dropped once it expires or no longer parses.
 */
export function signInDraftStore<Schema extends z.ZodType>(key: string, schema: Schema, maxAgeMs: number) {
  const Stored = z.object({ value: schema, returnTo: z.string(), expiresAt: z.number() });
  return {
    /** False when the browser refuses the write: the caller says so instead of losing the draft silently. */
    save(returnTo: string, value: z.input<Schema>): boolean {
      try {
        sessionStorage.setItem(key, JSON.stringify({ value, returnTo, expiresAt: Date.now() + maxAgeMs }));
        return true;
      } catch {
        return false;
      }
    },
    /** The draft saved for `returnTo`, removed once read; a draft saved for another URL stays for it. */
    take(returnTo: string): z.output<Schema> | null {
      try {
        const raw = sessionStorage.getItem(key);
        if (raw === null) return null;
        const result = Stored.safeParse(JSON.parse(raw));
        if (!result.success || result.data.expiresAt <= Date.now()) {
          sessionStorage.removeItem(key);
          return null;
        }
        if (result.data.returnTo !== returnTo) return null;
        sessionStorage.removeItem(key);
        return (result.data as { value: z.output<Schema> }).value;
      } catch {
        return null;
      }
    },
  };
}
