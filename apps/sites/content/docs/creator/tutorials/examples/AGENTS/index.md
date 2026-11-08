# Example scene catalog

This directory contains the source entries for the Example Scenes catalog. Each category has its own folder, and each example has its own Markdown file. These files are the source of truth.

`creator/tutorials/examples.md` and `catalog.json` are generated from the individual example files. Do not edit them directly. The generator also updates the category landing-page blocks and groups example pages by category in `creator/SUMMARY.md`. Keep the overview and categories at the same sidebar level, with examples nested one level below their category.

The tooling lives in `scripts/example-catalog/` at the repository root. Keep public data and images alongside the example content.

The generator derives `maintainer` from the GitHub source URL and `maintenance` as `decentraland` for the official organizations (`decentraland` and `decentraland-scenes`, run by the Decentraland Foundation, and `dcl-regenesislabs`, run by the Decentraland DAO) or `community` for other owners. The official list lives in `OFFICIAL_MAINTAINERS` in `scripts/example-catalog/frontmatter.mjs`. These describe repository ownership, not security review status. It also updates the source link, maintenance label, and community notice between `example-context` markers in every detail page. Do not edit those generated blocks; edit the source URL and regenerate instead. Keep descriptions educational and do not imply that inclusion certifies safety or grants permission to reuse code or assets.

Structure example pages as: page title, preview image (when available), project description, play/usage links, generated project information, then features and code highlights. Keep supporting screenshots beside the sections they illustrate; use the first representative preview at the top. Community notices use GitBook warning hints, including the Published Games overview and category landing page. Use level-two headings for main sections and level-three headings for individual code highlights. Preserve the generated block markers when moving sections.

Do not add this file to `creator/SUMMARY.md`. It is contributor guidance, not a catalog entry.

To enrich an example, inspect its implementation and add `highlights` to its YAML front matter. Each highlight requires `title`, `description`, `context` (project-specific dependencies or limitations), `tags`, and `references`. Each reference requires a `label` and an HTTPS GitHub file `url` pinned to a full 40-character commit SHA. Prefer line ranges when useful. The generator renders the “Explore the code” section between `example-highlights` markers and exports the same metadata in `catalog.json`; do not edit the rendered section. Keep the main source link pointed at upstream development. Highlights are learning references, not a security assessment.

The `license` field is a mapping. Use `{ status: unknown }` when this catalog has not identified applicable license information; this does not assert that no license exists. When upstream terms are identified, provide `status: identified`, a human-readable `name`, and an HTTPS `url` to the actual upstream license file or notice. Check the contents rather than copying GitHub's `NOASSERTION` label. Ownership does not establish licensing, and code and assets may have different terms. Check upstream terms before copying material. Catalog JSON schema version 2 adds structured license information and highlights.

When adding, removing, or changing an example entry:

1. Place the file in the folder for its category. The generator derives the category from the folder name.
2. Each category folder has a `README.md` with its display `title`, `description`, and `order`. Change that file to rename, describe, or reorder a category. Its body contains a generated category table between `example-category` markers; edit its metadata and regenerate. It is not an example entry.
3. Keep each example's YAML front matter valid and complete. Every page requires `id`, `name`, `description`, `tags`, `source`, and `license`.
4. Published games belong in `published-games` and include `developer`, derived from the GitHub repository owner.
	- Include `realm` with the World name from the deployed play link, such as `skychaser.dcl.eth`. For Genesis City deployments, use `position` with parcel coordinates, such as `-140,101`, instead.
5. Install dependencies with `npm ci --ignore-scripts` (Node.js 22 or newer), then run `npm run generate:examples` from the repository root before committing. Run `npm test` for validation tests and `npm run check:links` for repository-wide local links. CI runs `npm run check:examples` to ensure generated content is committed.
6. Commit generated changes to `catalog.json`, `creator/tutorials/examples.md`, `creator/SUMMARY.md`, category landing pages, and the detail-page context and highlight blocks with the entry change.
7. Reuse existing tags where they fit. Consult the `tags` arrays in [catalog.json](/docs/creator/tutorials/examples/catalog.json) for the current vocabulary.
