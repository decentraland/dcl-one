/** A docs page's Markdown source, for the Copy actions. A refused read releases its body before failing. */
export async function fetchDocMarkdown(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) {
    await response.body?.cancel().catch(() => {});
    throw new Error("Markdown unavailable");
  }
  return response.text();
}
