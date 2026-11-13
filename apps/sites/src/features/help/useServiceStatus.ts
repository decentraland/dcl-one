import { useQuery } from "@tanstack/react-query";

import { overallState, serviceStatusQuery, type Service, type OverallState, type ServiceResult } from "@/data/lib/help/status";

export type ServiceStatusReport = { readonly overall: OverallState; readonly services: readonly ServiceResult[] };

/** The pinged state of each service and the overall state; null until every ping has settled. */
export function useServiceStatus(services: readonly Service[]): ServiceStatusReport | null {
  const query = useQuery(serviceStatusQuery(services));
  if (!query.data) return null;
  return { overall: overallState(query.data), services: query.data };
}
