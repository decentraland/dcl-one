import { redactText } from '/shared/redact';

/** `text` with secrets and personal data masked, for the log and for log attachments. */
export function redactSecrets(text: string): string {
  // One set of rules for logs and for shared turns, so the two can't drift apart.
  return redactText(text).text;
}
