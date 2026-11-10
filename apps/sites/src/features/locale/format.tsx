import { Fragment, type ReactNode } from "react";

export type MessageValue = ReactNode | ((chunks: ReactNode) => ReactNode);
export type MessageValues = Record<string, MessageValue>;

// The catalog syntax these pages use: `{name}` placeholders and simple `<tag>…</tag>` markup.
// Everything renders as React text or the caller's elements; catalog text is never parsed as HTML.
const TOKEN = /<(\w+)>([\s\S]*?)<\/\1>|\{(\w+)\}/g;

export function formatMessage(
  message: string,
  values: MessageValues = {},
): ReactNode {
  const parts: ReactNode[] = [];
  let last = 0;
  for (const match of message.matchAll(TOKEN)) {
    if (match.index > last) parts.push(message.slice(last, match.index));
    const [whole, tag, inner, name] = match;
    if (tag !== undefined) {
      const content = formatMessage(inner ?? "", values);
      const handler = values[tag];
      parts.push(typeof handler === "function" ? handler(content) : content);
    } else if (name !== undefined) {
      const value = values[name];
      parts.push(
        value === undefined || typeof value === "function" ? whole : value,
      );
    }
    last = match.index + whole.length;
  }
  if (last < message.length) parts.push(message.slice(last));
  if (
    parts.every((part) => typeof part === "string" || typeof part === "number")
  )
    return parts.join("");
  return parts.map((part, index) => <Fragment key={index}>{part}</Fragment>);
}
