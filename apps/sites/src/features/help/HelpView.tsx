import { HelpAccordion } from "@dcl/one-ui/apps/sites/help/HelpAccordion";
import { HelpChatBanner } from "@dcl/one-ui/apps/sites/help/HelpChatBanner";
import { HelpPage } from "@dcl/one-ui/apps/sites/help/HelpPage";
import { HelpSection } from "@dcl/one-ui/apps/sites/help/HelpSection";
import { HelpStatus } from "@dcl/one-ui/apps/sites/help/HelpStatus";
import type { HelpTab } from "@dcl/one-ui/apps/sites/help/HelpTabs";
import { useState } from "react";

import type { Service } from "@/data/lib/help/status";

import {
  CREATOR_FUND_CONTACT_EMAIL,
  CREATOR_FUND_CONTACT_PROMPT,
  CREATOR_FUND_CONTACT_TRACKING_NAME,
  CREATOR_FUND_FAQ,
} from "../creator-fund/faq";
import { FULL_FAQ_URL, HELP_COPY, HELP_FAQ, HELP_SUPPORT_UPDATES } from "./content";
import { showIntercom, useIntercom, type IntercomSettings } from "./intercom";
import { useServiceStatus } from "./useServiceStatus";

export type HelpTabId = "faq" | "creator-fund" | "support-updates";

const TABS: readonly HelpTab[] = [
  { id: "faq", label: HELP_COPY.tabs.faq, icon: "faq" },
  { id: "creator-fund", label: HELP_COPY.tabs.creatorFund, icon: "creatorFund" },
  { id: "support-updates", label: HELP_COPY.tabs.supportUpdates, icon: "supportUpdates" },
];

const INTERCOM_SETTINGS: IntercomSettings = { alignment: "right" };

const faqItems = HELP_FAQ.map((item, index) => ({ id: `faq-${index}`, ...item }));
const supportItems = HELP_SUPPORT_UPDATES.map((item, index) => ({ id: `support-${index}`, ...item }));
const creatorFundItems = CREATOR_FUND_FAQ.map(({ id, question, answer }) => ({ id, question, answer }));
const creatorFundTracking = new Map(CREATOR_FUND_FAQ.map((entry) => [entry.id, entry.trackingName]));

export type HelpViewProps = {
  readonly services: readonly Service[];
  readonly statusPageUrl: string;
  /** Empty: no chat widget. */
  readonly intercomAppId: string;
  readonly wallet?: string | null;
  /** A Creator Fund control was used: a question opened, or the contact address clicked. */
  readonly onCreatorFundClick?: (button: string) => void;
  readonly initialTab?: HelpTabId;
};

function CreatorFundSection({ onClick }: { onClick?: (button: string) => void }) {
  return (
    <HelpSection
      title={HELP_COPY.creatorFund.title}
      description={HELP_COPY.creatorFund.paragraph}
      footer={
        <>
          {CREATOR_FUND_CONTACT_PROMPT}{" "}
          <a
            href={`mailto:${CREATOR_FUND_CONTACT_EMAIL}`}
            onClick={() => onClick?.(CREATOR_FUND_CONTACT_TRACKING_NAME)}
          >
            {CREATOR_FUND_CONTACT_EMAIL}
          </a>
        </>
      }
    >
      <HelpAccordion
        items={creatorFundItems}
        onOpen={(id) => {
          const name = creatorFundTracking.get(id);
          if (name) onClick?.(name);
        }}
      />
    </HelpSection>
  );
}

export function HelpView({
  services,
  statusPageUrl,
  intercomAppId,
  wallet,
  onCreatorFundClick,
  initialTab = "faq",
}: HelpViewProps) {
  const [tab, setTab] = useState<HelpTabId>(initialTab);
  const report = useServiceStatus(services);
  const chatAvailable = useIntercom(intercomAppId, INTERCOM_SETTINGS, wallet);

  return (
    <HelpPage
      title={HELP_COPY.title}
      description={HELP_COPY.description}
      tabs={TABS}
      activeTab={tab}
      onTabChange={(id) => setTab(id as HelpTabId)}
      status={
        <HelpStatus
          label={HELP_COPY.status.title}
          report={
            report && {
              overall: report.overall,
              overallText: HELP_COPY.status[report.overall],
              services: report.services,
            }
          }
          detailsLabel={HELP_COPY.status.details}
          detailsHref={statusPageUrl}
        />
      }
      banner={
        // No chat widget (switched off, or its script failed): no button that would do nothing.
        chatAvailable && (
        <HelpChatBanner
          title={HELP_COPY.chat.title}
          subtitle={HELP_COPY.chat.subtitle}
          actionLabel={HELP_COPY.chat.button}
          onAction={showIntercom}
        />
        )
      }
    >
      {tab === "faq" ? (
        <HelpSection
          key="faq"
          title={HELP_COPY.faq.title}
          description={
            <>
              {HELP_COPY.faq.paragraph} <a href={FULL_FAQ_URL}>{HELP_COPY.faq.ctaLink}.</a>
            </>
          }
        >
          <HelpAccordion items={faqItems} />
        </HelpSection>
      ) : tab === "creator-fund" ? (
        <CreatorFundSection key="creator-fund" onClick={onCreatorFundClick} />
      ) : (
        <HelpSection key="support-updates" title={HELP_COPY.supportUpdates.title}>
          <HelpAccordion items={supportItems} />
        </HelpSection>
      )}
    </HelpPage>
  );
}
