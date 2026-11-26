/**
 * The Creator Fund FAQ: one copy for every page that shows it (the Creator Fund tab of /help today, the
 * /creator-fund page once it migrates). Copy from decentraland/sites `page.creator_fund.faq.*` (en.json), in the
 * order of `faqEntries` (src/components/CreatorFund/data.ts). `trackingName` is the stable English name a click
 * on the question is tracked under.
 */

export type CreatorFundFaqEntry = {
  readonly id: string;
  readonly question: string;
  readonly answer: string;
  readonly trackingName: string;
};

export const CREATOR_FUND_FAQ_TITLE = "FAQ";

export const CREATOR_FUND_FAQ: readonly CreatorFundFaqEntry[] = [
  {
    id: "join",
    trackingName: "How Do I Join",
    question: "How Do I Join?",
    answer:
      "Opt-in opens in Creator Hub on November 11. You must be at least 18 and own the LAND, NAME or World where your experience is deployed. Sign the Program Terms with the owning wallet; one opt-in covers every eligible LAND, NAME and World owned by that wallet.",
  },
  {
    id: "rewards",
    trackingName: "How Are Payouts Calculated",
    question: "How Are Rewards Decided?",
    answer:
      "Each cycle’s allocation is shared among qualifying experiences based on active playtime, player retention and new Decentraland players. Creator Hub metrics are informational; final rewards are calculated after review and further filtering, so the figures used may differ from your dashboard.",
  },
  {
    id: "worlds",
    trackingName: "Do Worlds Count",
    question: "Do Worlds Count?",
    answer: "Yes. Scenes in Genesis City and Worlds both count.",
  },
  {
    id: "wallet",
    trackingName: "Custom Payout Wallet",
    question: "Can I Use A Custom Payout Wallet?",
    answer:
      "Yes. Rewards are paid to the Payout Wallet you set in Creator Hub, which can be any wallet or exchange account you control. If you haven’t set one 7 days after a Cycle ends, the reward payout is lost.",
  },
  {
    id: "transfer",
    trackingName: "Transfer During A Cycle",
    question: "What If I Transfer My LAND, NAME Or World During A Cycle?",
    answer:
      "If you transfer it during a cycle, that cycle’s activity doesn’t count for anyone. The new owner counts from the next cycle. Rewards for cycles completed before the transfer are still paid to the Payout Wallet on record for each of those cycles. If you’re moving it to another wallet you control, tell us before the transfer and we may treat it as a continuation of your participation.",
  },
  {
    id: "runs",
    trackingName: "Who Runs The Creator Fund",
    question: "Who Runs The Creator Fund?",
    answer:
      "The Creator Fund was created and is funded by DCL Regenesis Labs Foundation and the Decentraland Foundation. It is operated by the Decentraland Foundation.",
  },
  {
    id: "after",
    trackingName: "After The Beta Season",
    question: "What Happens After The Beta Season?",
    answer: "Season 1 is planned for January–March 2027. Dates and updates will be confirmed December 28.",
  },
  {
    id: "grant",
    trackingName: "Is This A Grant",
    question: "Is This A Grant?",
    answer:
      "No. Grants support projects before they launch. The Creator Fund rewards experiences that are already live and growing on Decentraland.",
  },
];

export const CREATOR_FUND_CONTACT_EMAIL = "creatorfund@decentraland.org";
export const CREATOR_FUND_CONTACT_PROMPT = "Questions or need support? Email us at";
/** The tracking name of a click on the contact address. */
export const CREATOR_FUND_CONTACT_TRACKING_NAME = "Email Creator Fund";
