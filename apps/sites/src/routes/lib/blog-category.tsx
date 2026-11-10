import { categorySeo } from "@/features/blog/seo";

import { useParams } from "react-router";
import { listQuery, BlogMissingError } from "@/data/lib/blog/options";
import { PostList } from "@/features/blog/BlogShell";
import { BlogRoute } from "@/features/blog/BlogRoute";
import {
  serverBase,
  type BlogDocument,
  type BlogBase,
} from "@/features/blog/loaders";
import { BlogCategoryHero } from "@dcl/one-ui/apps/sites/blog/BlogListHeaders";
import { BlogPage, useBlogNavigate } from "./blog-page";
export async function categoryLoader({
  request,
  params,
}: {
  request: Request;
  params: { slug?: string; categorySlug?: string };
}) {
  const slug = params.categorySlug ?? params.slug ?? "";
  const { client, base, wrap, required } = await serverBase(
    request,
    "blog_category",
  );
  await required(async () => {
    if (!base.categories.some((item) => item.slug === slug))
      throw new BlogMissingError("category");
    return client.fetchInfiniteQuery(listQuery({ scope: "category", slug }));
  });
  return wrap({ publicBase: base.publicBase });
}
export const categoryClientLoader = () => null;
function CategoryView({ base }: { base: BlogBase }) {
  const params = useParams();
  const slug = params.categorySlug ?? params.slug ?? "";
  const category = base.categories.find((item) => item.slug === slug);
  const navigate = useBlogNavigate();
  if (!category) throw new BlogMissingError("category");
  return (
    <BlogPage
      base={base}
      seo={categorySeo(category, base.publicBase)}
      activeHref={category.url}
      banner={
        <BlogCategoryHero
          title={category.title}
          description={category.description}
          image={category.image.url}
        />
      }
    >
      <PostList
        options={listQuery({ scope: "category", slug })}
        hasMainPost
        onNavigate={navigate}
      />
    </BlogPage>
  );
}
export function BlogCategoryPage({ data }: { data: BlogDocument | null }) {
  return (
    <BlogRoute data={data}>{(base) => <CategoryView base={base} />}</BlogRoute>
  );
}
