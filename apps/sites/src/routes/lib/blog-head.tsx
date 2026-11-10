import { blogMeta, type BlogSeo } from "@/features/blog/route-kit";

/** React hoists route-owned metadata for the server document and cache-driven navigation alike. */
export function BlogHead({
  seo,
  noIndex = false,
}: {
  seo: BlogSeo;
  noIndex?: boolean;
}) {
  return (
    <>
      {blogMeta(seo).map((descriptor, index) => {
        if ("title" in descriptor)
          return <title key={index}>{descriptor.title}</title>;
        if (descriptor.tagName === "link")
          return (
            <link key={index} rel={descriptor.rel} href={descriptor.href} />
          );
        return <meta key={index} {...descriptor} />;
      })}
      {noIndex ? <meta name="robots" content="noindex" /> : null}
    </>
  );
}
