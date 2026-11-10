import {
  categoryLoader,
  categoryClientLoader,
  BlogCategoryPage,
} from "../lib/blog-category";
import { BlogErrorBoundary, blogHeaders } from "../lib/blog-page";
import type { BlogDocument } from "@/features/blog/loaders";
import type { Route } from "./+types/blog.$slug";
export const loader = ({ request, params }: Route.LoaderArgs) =>
  categoryLoader({ request, params });
export const clientLoader = categoryClientLoader;
export default function CategoryRoute({ loaderData }: Route.ComponentProps) {
  return <BlogCategoryPage data={loaderData as BlogDocument | null} />;
}
export const ErrorBoundary = BlogErrorBoundary;
export const headers = blogHeaders;

export const meta = () => [];
