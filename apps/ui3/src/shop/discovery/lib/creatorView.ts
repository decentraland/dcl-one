// from: shop@2d3bf62:app/src/pages/Creator.view.ts

export type GridView = "skeletons" | "items" | "error" | "no-creations" | "filters";

export function resolveGridView(input: {
  gridLoading: boolean;
  gridError: boolean;
  gridCount: number;
  baselinePending: boolean;
  baselineError: boolean;
  baselineCount: number | undefined;
}): GridView {
  const { gridLoading, gridError, gridCount, baselinePending, baselineError, baselineCount } = input;
  if (gridLoading) return "skeletons";
  if (gridError) return "error";
  if (gridCount > 0) return "items";
  if (baselinePending) return "skeletons";
  if (baselineError) return "error";
  return baselineCount === 0 ? "no-creations" : "filters";
}
