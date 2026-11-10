// A BlogSource over the committed CMS fixture (fixtures/cms-sample.json): tests use it, and local dev
// may (see current-source.ts). It must never reach a production bundle; a build check asserts that.
import fixture from "./fixtures/cms-sample.json";
import {
  mapAuthor,
  mapCategory,
  mapPage,
  mapPost,
  mapSearchPage,
  type CmsEntry,
  type CmsLookup,
} from "./normalize";
import type { BlogSource } from "./source";

type Fixture = {
  posts: CmsEntry[];
  categories: CmsEntry[];
  authors: CmsEntry[];
  assets: Record<string, CmsEntry>;
};

export function createFixtureBlogSource(
  data: Fixture = fixture as unknown as Fixture,
): BlogSource {
  const lookup: CmsLookup = {
    entries: new Map(
      [...data.categories, ...data.authors].map((entry) => [
        entry.sys.id,
        entry,
      ]),
    ),
    assets: new Map(Object.entries(data.assets)),
  };
  const posts = () =>
    data.posts.map((entry) => ({ entry, post: mapPost(entry, lookup)! }));
  const page = (entries: CmsEntry[], param: { skip: number; limit: number }) =>
    mapPage(
      {
        items: entries.slice(param.skip, param.skip + param.limit),
        total: entries.length,
      },
      param,
      lookup,
    );

  return {
    async listPosts(scope, param, signal) {
      signal.throwIfAborted();
      const matches = posts().filter(({ post }) =>
        scope.scope === "home"
          ? true
          : scope.scope === "category"
            ? post.category.slug === scope.slug
            : post.author.slug === scope.slug,
      );
      return page(
        matches.map(({ entry }) => entry),
        param,
      );
    },
    async searchPosts(q, param, signal) {
      signal.throwIfAborted();
      const needle = q.toLowerCase();
      const matches = posts().filter(({ post }) =>
        `${post.title} ${post.description}`.toLowerCase().includes(needle),
      );
      // The sample holds no search answers, so hits carry no _highlight: each falls back to its fields.
      return mapSearchPage(
        {
          items: matches
            .slice(param.skip, param.skip + param.limit)
            .map(({ entry }) => entry),
          total: matches.length,
        },
        param,
        lookup,
      );
    },
    async getPost(categorySlug, postSlug, signal) {
      signal.throwIfAborted();
      return (
        posts().find(
          ({ post }) =>
            post.slug === postSlug && post.category.slug === categorySlug,
        )?.post ?? null
      );
    },
    async listCategories(signal) {
      signal.throwIfAborted();
      return data.categories
        .map((entry) => mapCategory(entry, lookup))
        .filter((category) => category !== null);
    },
    async getAuthor(slug, signal) {
      signal.throwIfAborted();
      const entry = data.authors.find(
        (author) => mapAuthor(author, lookup).slug === slug,
      );
      return entry ? mapAuthor(entry, lookup) : null;
    },
  };
}
