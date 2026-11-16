// Ported verbatim from decentraland/sites@cfdfa1a5 (src/pages/security); the text is legal content:
// do not edit it by hand. Its frozen rendering is apps/sites/src/data/legal/fixtures/security.json.
import type { LegalDocument } from "./types";

export const SECURITY: LegalDocument = {
  key: "security",
  slug: "security",
  title: "Vulnerability Disclosure Procedure",
  pageTitle: "Security | Decentraland",
  toc: [],
  intro: [
    {
      type: "p",
      content: [
        "At Decentraland, we take every measure necessary to ensure the security of the platform. If you are a security researcher and took a look at some of our code, contracts, or websites and found a vulnerability, please report it responsibly by emailing us at security@decentraland.org.",
      ],
    },
  ],
  sections: [],
  cta: {
    href: "mailto:security@decentraland.org",
    label: "Report a security issue",
  },
};
