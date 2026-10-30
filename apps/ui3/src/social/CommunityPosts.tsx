import { useEffect, useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Message, oldestFirst, type SocialDetail, type SocialPost } from "../explorer/pages/Communities";
import CompactNavigation from "../explorer/components/CompactNavigation";
import ConfirmDialog from "../explorer/components/ConfirmDialog";

function newPublishKey(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}

export default function CommunityPosts({ detail, viewer, postId, load, write }: {
  detail: SocialDetail;
  viewer: string | null;
  postId?: string | null;
  load: (offset: number, signal: AbortSignal) => Promise<{ posts: SocialPost[]; total: number }>;
  write: (method: string, suffix: string, body?: object, headers?: Record<string, string>) => Promise<void>;
}) {
  const [composing, setComposing] = useState(false);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<SocialPost | null>(null);
  const [deleting, setDeleting] = useState<SocialPost | null>(null);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const publishKey = useRef<{ content: string; key: string } | null>(null);
  const [error, setError] = useState("");
  const role = detail.community?.role;
  const canPost = !!viewer && ["owner", "moderator", "mod"].includes(role ?? "");
  const feed = useInfiniteQuery({
    queryKey: ["web-social", viewer, detail.id, "posts"], initialPageParam: 0, retry: false,
    queryFn: ({ pageParam, signal }) => load(pageParam, signal),
    getNextPageParam: (last, pages) => {
      const offset = pages.reduce((count, page) => count + page.posts.length, 0);
      return last.posts.length && offset < last.total ? offset : undefined;
    },
  });
  const posts = feed.isError ? [] : [...new Map(feed.data?.pages.flatMap(page => page.posts).map(post => [post.id, post])).values()];
  useEffect(() => { if (postId) document.getElementById(`post-${postId}`)?.scrollIntoView({ block: "center" }); }, [postId, feed.data, feed.isError]);
  async function submit(method: string, suffix: string, body?: object) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setError("");
    const publishing = method === "POST" && suffix === "";
    if (publishing) {
      const content = (body as { content?: string } | undefined)?.content ?? "";
      if (publishKey.current?.content !== content) publishKey.current = { content, key: newPublishKey() };
    }
    try {
      if (publishing && publishKey.current) await write(method, suffix, body, { "Idempotency-Key": publishKey.current.key });
      else await write(method, suffix, body);
      if (publishing) publishKey.current = null;
      if (method === "PATCH" || publishing) { setDraft(""); setComposing(false); setEditing(null); }
      setDeleting(null);
    } catch (error) {
      void feed.refetch();
      setError(error instanceof Error && "status" in error && (error.status === 401 || error.status === 403)
        ? "You no longer have permission to change posts in this community."
        : error instanceof Error ? error.message : "The post could not be updated. Try again.");
    } finally { pending.current = false; setBusy(false); }
  }
  return <section className="soc__page soc-posts" aria-label="Community posts">
    <header className="soc__pagehead"><h1>Announcements</h1>
      {canPost && !composing && <button className="soc__primary" type="button" onClick={() => { setError(""); setEditing(null); setDraft(""); setComposing(true); }}>New post</button>}
    </header>
    {error && <p role="alert">{error}</p>}
    {composing && canPost && <form aria-label={editing ? "Edit post" : "New post"} onSubmit={event => { event.preventDefault(); void submit(editing ? "PATCH" : "POST", editing ? `/${editing.id}` : "", { content: draft }); }}>
      <label>Post text<textarea autoFocus required value={draft} disabled={busy} onChange={event => setDraft(event.target.value)} /></label>
      <div className="soc-posts__actions">
        <button className="soc__primary" type="submit" disabled={busy || !draft.trim()}>{busy ? "Saving…" : editing ? "Save changes" : "Publish post"}</button>
        <button className="soc__outline" type="button" disabled={busy} onClick={() => { setDraft(""); setEditing(null); setComposing(false); setError(""); }}>Cancel</button>
      </div>
    </form>}
    {!canPost && <p className="soc__muted">Only owners and moderators can publish announcements.</p>}
    {feed.isPending && <p role="status">Loading announcements…</p>}
    {feed.isError && <p role="alert">{feed.error.message} <button className="soc__outline" type="button" onClick={() => void feed.refetch()}>Retry posts</button></p>}
    {feed.hasNextPage && !feed.isError && <button className="soc__outline" type="button" disabled={busy || feed.isFetching} onClick={() => void feed.fetchNextPage()}>
      {feed.isFetchingNextPage ? "Loading older posts…" : "Load older posts"}
    </button>}
    <div className="soc__thread" aria-label="Posts">
      {postId && feed.isSuccess && !posts.some(post => post.id === postId) && <p role="status">{feed.hasNextPage ? "Load older posts to find this announcement." : "This announcement is unavailable."}</p>}
      {feed.isSuccess && !posts.length && <p className="soc__muted">No announcements yet.</p>}
      {oldestFirst(posts).map(post => <Message key={post.id} post={post}>
        <div className="soc-posts__actions">
          <a className="soc__cardlink" href={`/social?community=${encodeURIComponent(detail.id)}&post=${encodeURIComponent(post.id)}`}>Post link</a>
          <span aria-label="Likes">{post.likesCount} {post.likesCount === 1 ? "like" : "likes"}</span>
          {viewer && role !== "banned" && <button type="button" className="soc__outline" aria-pressed={!!post.isLikedByUser} disabled={busy}
            onClick={() => void submit(post.isLikedByUser ? "DELETE" : "POST", `/${post.id}/like`)}>{post.isLikedByUser ? "Unlike" : "Like"}</button>}
          {canPost && (role === "owner" || post.authorAddress.toLowerCase() === viewer?.toLowerCase()) && <CompactNavigation label="Actions" name="Post actions"
            items={[
              ...(post.authorAddress.toLowerCase() === viewer?.toLowerCase() ? [{ kind: "button" as const, label: "Edit post", disabled: busy || composing,
                onClick: () => { setError(""); setEditing(post); setDraft(post.content); setComposing(true); } }] : []),
              { kind: "button", label: "Delete post", danger: true, disabled: busy || composing, onClick: () => { setError(""); setDeleting(post); } },
            ]} />}
        </div>
      </Message>)}
    </div>
    {deleting && <ConfirmDialog title="Delete post?" body={error || "This announcement will be removed for everyone."} danger
      confirmLabel={busy ? "Deleting…" : "Delete post"} cancelLabel="Keep post"
      onConfirm={() => void submit("DELETE", `/${deleting.id}`)} onCancel={() => { if (!busy) setDeleting(null); }} />}
  </section>;
}
