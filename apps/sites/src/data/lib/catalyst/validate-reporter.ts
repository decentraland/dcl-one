import { setValidationReporter } from "@web-core/validate";

import { reportSchemaDrift } from "./schema-drift";

export function installValidationReporter(): void {
  setValidationReporter(({ boundary, paths }) => {
    reportSchemaDrift(boundary, paths);
  });
}
