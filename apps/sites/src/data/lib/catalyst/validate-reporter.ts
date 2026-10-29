import { setValidationReporter } from "@ui/validate";

import { reportSchemaDrift } from "./schema-drift";

export function installValidationReporter(): void {
  setValidationReporter(({ boundary, paths }) => {
    reportSchemaDrift(boundary, paths);
  });
}
