// Ported verbatim from decentraland/sites@b7e4fa8c (src/pages/creator-fund-terms); the text is legal
// content: do not edit it by hand. Its frozen rendering is apps/sites/src/data/legal/fixtures/creator-fund-terms.json.
import type { LegalDocument } from "./types";

export const CREATOR_FUND: LegalDocument = {
  key: "creatorFund",
  slug: "creator-fund-terms",
  title: "Creator Fund",
  pageTitle: "Creator Fund - Terms",
  toc: [
    {
      id: "about-the-program",
      label: "1. About the Program and these Terms",
    },
    {
      id: "nature-of-rewards",
      label: "2. Nature of Rewards",
    },
    {
      id: "eligibility",
      label: "3. Eligibility",
    },
    {
      id: "enrollment-and-payout-wallet",
      label: "4. Enrollment and Payout Wallet",
    },
    {
      id: "qualifying-experiences-and-players",
      label: "5. Qualifying Experiences and Qualifying Players",
    },
    {
      id: "reward-calculation",
      label: "6. Reward Calculation",
    },
    {
      id: "payment",
      label: "7. Payment",
    },
    {
      id: "fair-play-and-exclusion",
      label: "8. Fair Play and Exclusion",
    },
    {
      id: "taxes",
      label: "9. Taxes",
    },
    {
      id: "data-and-publicity",
      label: "10. Data and Publicity",
    },
    {
      id: "changes-suspension-and-termination",
      label: "11. Changes, Suspension and Termination",
    },
    {
      id: "disclaimers-liability-and-disputes",
      label: "12. Disclaimers, Liability and Disputes",
    },
  ],
  intro: [
    {
      type: "p",
      content: ["Terms and Conditions (Beta Season)"],
    },
  ],
  sections: [
    {
      id: "about-the-program",
      blocks: [
        {
          type: "h2",
          content: ["1. About the Program and these Terms"],
        },
        {
          type: "p",
          content: [
            "1.1 The ",
            {
              type: "link",
              href: "/creator-fund",
              children: ["Decentraland Creator Fund"],
            },
            ' (the "',
            {
              type: "strong",
              children: ["Program"],
            },
            '") is a reward program operated by Decentraland Foundation (the "',
            {
              type: "strong",
              children: ["Foundation"],
            },
            '", "',
            {
              type: "strong",
              children: ["we"],
            },
            '", "',
            {
              type: "strong",
              children: ["us"],
            },
            '"). It grants rewards ("Creator Rewards", or "Rewards" in these Terms) to creators whose Decentraland experiences attract and retain real players, measured against the criteria published in these Terms and on the ',
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            '. In Program communications, Rewards may also be referred to as "payouts" or "Creator Fund payouts". This does not change their nature under Section 2.',
          ],
        },
        {
          type: "p",
          content: [
            '1.2 These Terms govern the Beta Season of the Program divided into payout periods (each a "',
            {
              type: "strong",
              children: ["Cycle"],
            },
            '") as set out on the ',
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ". All Cycle dates and times are in UTC.",
          ],
        },
        {
          type: "p",
          content: [
            "1.3 You join the Program by opting in through Creator Hub and signing these Terms with the wallet that owns your LAND, NAME or World. By signing, you confirm that you have read and accept these Terms, the Decentraland Terms of Use (",
            {
              type: "link",
              href: "/terms",
              children: ["decentraland.org/terms"],
            },
            ', the "Terms of Use"), the Content Policy (',
            {
              type: "link",
              href: "/content",
              children: ["decentraland.org/content"],
            },
            "), the Privacy Policy (",
            {
              type: "link",
              href: "/privacy",
              children: ["decentraland.org/privacy"],
            },
            ") and the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ", which form part of these Terms. These Terms are additional terms of participation issued by the Foundation under Section 1.3 of the Terms of Use. Under Section 19.1 of the Terms of Use, they prevail over the Terms of Use to the extent of any conflict on matters relating to the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "1.4 If you do not accept these Terms, do not opt in. Your scene remains in Decentraland as usual; it simply does not take part in the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "1.5 Capitalised terms not defined in these Terms have the meaning given to them in the Decentraland Terms of Use.",
          ],
        },
        {
          type: "p",
          content: [
            "1.6 ",
            {
              type: "strong",
              children: ["These Terms apply only to the Creator Fund."],
            },
            " Any other Decentraland program, now or in the future (for example the ",
            {
              type: "link",
              href: "/rewards-terms",
              children: ["Rewards Program"],
            },
            ", ",
            {
              type: "link",
              href: "/credits-terms",
              children: ["Credits"],
            },
            " or the ",
            {
              type: "link",
              href: "/referral-terms",
              children: ["Referral Program"],
            },
            "), is separate and governed by its own terms. Those terms do not apply to the Creator Fund, and nothing earned under them counts toward Creator Rewards, unless the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ' says otherwise. In these Terms, "Rewards" and "Season" refer only to Creator Fund rewards and seasons.',
          ],
        },
      ],
    },
    {
      id: "nature-of-rewards",
      blocks: [
        {
          type: "h2",
          content: ["2. Nature of Rewards"],
        },
        {
          type: "p",
          content: [
            "2.1 ",
            {
              type: "strong",
              children: ["Rewards."],
            },
            " Rewards recognise results your experience has already achieved, measured after the end of each Cycle under Section 6. They are not payment for any work, service or deliverable. You are free to build, update, promote or stop running your experience as you see fit, and nothing in these Terms requires you to do anything.",
          ],
        },
        {
          type: "p",
          content: [
            "2.2 ",
            {
              type: "strong",
              children: ["No relationship beyond these Terms."],
            },
            " Taking part in the Program does not create any employment, contractor, agency, partnership or joint-venture relationship between you and the Foundation, the DAO, DCL Regenesis Labs Foundation or any other Decentraland entity. We do not direct or control how you create or operate your experience, beyond the rules that apply to every Decentraland user. Rewards are not wages or fees. You decide whether, when and how to work on your experience, without supervision or direction from us, using your own equipment and resources.",
          ],
        },
        {
          type: "p",
          content: [
            "2.3 ",
            {
              type: "strong",
              children: ["No guaranteed income."],
            },
            " The Program does not promise any level of income. Rewards depend on measured performance and on the size of the Season Pool, and may be zero in any Cycle.",
          ],
        },
        {
          type: "p",
          content: [
            "2.4 ",
            {
              type: "strong",
              children: ["No entitlement before it is earned."],
            },
            " A Reward is earned only when it is calculated and confirmed for a Cycle under these Terms. Before then, you have no right to any amount.",
          ],
        },
        {
          type: "p",
          content: [
            "2.5 ",
            {
              type: "strong",
              children: ["No chance element."],
            },
            " Rewards are determined solely by measured performance under Section 6. No part of the Program involves a draw, raffle or random selection.",
          ],
        },
      ],
    },
    {
      id: "eligibility",
      blocks: [
        {
          type: "h2",
          content: ["3. Eligibility"],
        },
        {
          type: "p",
          content: [
            "3.1 To take part, you must, at opt-in and for as long as you take part in the Program:",
          ],
        },
        {
          type: "ol",
          items: [
            [
              "be at least 18 years old, or the age of legal majority in your jurisdiction if higher (Terms of Use, Section 8);",
            ],
            ["own the LAND, NAME or World where your experience is deployed;"],
            ["not be an Excluded Person under Section 3.2;"],
            [
              "be resident in a place where we can lawfully pay you in the currency you elected;",
            ],
            [
              "not be subject to sanctions imposed by the United Nations, the United States, the United Kingdom or the European Union, or act on behalf of someone who is.",
            ],
          ],
        },
        {
          type: "p",
          content: [
            "3.2 ",
            {
              type: "strong",
              children: ["Excluded Persons."],
            },
            " The following may not take part: (a) persons who design, operate or make decisions on the Program, or have access to its non-public data or detection methods; (b) consultants, contractors and advisors of the Foundation or of DCL Regenesis Labs Foundation, except creators engaged under the Creator Success Program, who may take part in respect of their own experiences.",
          ],
        },
        {
          type: "p",
          content: [
            "3.3 ",
            {
              type: "strong",
              children: ["Declarations."],
            },
            " At opt-in you declare your contact name, email, and that you meet Section 3.1. If any declaration is or becomes untrue, you must tell us immediately and you stop being eligible. You must confirm your email when you set your Payout Wallet under Section 4.2.",
          ],
        },
        {
          type: "p",
          content: [
            "3.4 ",
            {
              type: "strong",
              children: ["Verification."],
            },
            " We may ask you to verify your identity, age or residence at any time, and will do so before paying further Rewards once your total Rewards in the Season exceed USD 5,000 or if we detect signals of fraud. If you do not complete verification within 15 days, we cancel unpaid Rewards.",
          ],
        },
        {
          type: "p",
          content: [
            "3.5 ",
            {
              type: "strong",
              children: ["Entities."],
            },
            ' If you accept these Terms on behalf of a company or other entity, you confirm that you are authorised to bind it, and "you" means that entity. Both the entity and the persons who control it must meet Section 3.1.',
          ],
        },
      ],
    },
    {
      id: "enrollment-and-payout-wallet",
      blocks: [
        {
          type: "h2",
          content: ["4. Enrollment and Payout Wallet"],
        },
        {
          type: "p",
          content: [
            "4.1 ",
            {
              type: "strong",
              children: ["Owner Wallet and opt-in."],
            },
            ' The wallet that owns your LAND, NAME or World (the "Owner Wallet") signs these Terms and is the participant in the Program. One opt-in covers every eligible LAND, NAME and World owned by that wallet, including any experience you publish later, which joins when you next open the Creator Fund tab in Creator Hub. If you own LAND, NAMEs or Worlds through more than one wallet, each wallet needs its own opt-in.',
          ],
        },
        {
          type: "p",
          content: [
            "4.2 ",
            {
              type: "strong",
              children: ["Payout Wallet."],
            },
            " Rewards are paid to the Payout Wallet you set in Creator Hub, which must be a wallet or exchange account you control. If you haven't set one within 7 days after a Cycle ends, the Reward for that Cycle is not earned and is not paid.",
          ],
        },
        {
          type: "p",
          content: [
            "4.3 ",
            {
              type: "strong",
              children: ["Responsibility for wallets."],
            },
            " You are responsible for the security of your wallets and for giving us a correct address that can receive USDC as an ERC-20 token on Ethereum mainnet. Payments sent to the address on record are final, and we are not responsible for losses caused by an incorrect address or a compromised wallet. Tell us promptly if you suspect that someone has accessed your account or changed your Payout Wallet without your permission.",
          ],
        },
        {
          type: "p",
          content: [
            "4.4 ",
            {
              type: "strong",
              children: ["Teams and collaborators."],
            },
            " If others help build or run your experience, the Owner Wallet is the only participant. We pay only to the Payout Wallet on record, and how Rewards are shared is solely a matter between you and your collaborators. You are responsible for complying with any law that applies to that arrangement, including labour and tax law where your collaborators live. Collaborators, deployers and studios have no claim against the Foundation, and their only remedy in any payment dispute is against you.",
          ],
        },
        {
          type: "p",
          content: [
            "4.5 ",
            {
              type: "strong",
              children: ["Transfer of a LAND, NAME or World."],
            },
            " If a LAND, NAME or World is transferred during a Cycle, the Owner Wallet's participation for that LAND, NAME or World ends on the date of transfer, and that experience's activity in that Cycle does not count toward Rewards for anyone. Rewards for Cycles completed before the transfer remain payable to the Payout Wallet on record for each of those Cycles. Participation does not pass to the new owner, who may opt in and will count from the next Cycle. If you are moving the LAND, NAME or World to another wallet you control, tell us before the transfer. We may, at our discretion, treat it as a continuation of your participation.",
          ],
        },
        {
          type: "p",
          content: [
            "4.6 ",
            {
              type: "strong",
              children: ["Joining during a Cycle."],
            },
            " If you opt in during a Cycle, the whole Cycle counts toward your Rewards, including activity before your opt-in, provided that you opt in before the Cycle ends and have owned the LAND, NAME or World for the entire Cycle. Otherwise, your participation counts from the next Cycle. These Terms, including Sections 5 and 8, apply to that earlier activity as if you had opted in when the Cycle started. Activity from earlier Cycles or before the Season does not count.",
          ],
        },
      ],
    },
    {
      id: "qualifying-experiences-and-players",
      blocks: [
        {
          type: "h2",
          content: ["5. Qualifying Experiences and Qualifying Players"],
        },
        {
          type: "p",
          content: [
            "5.1 ",
            {
              type: "strong",
              children: ["Qualifying Experience."],
            },
            " An experience counts in a Cycle only if, throughout that Cycle, it complies with the ",
            {
              type: "link",
              href: "/terms",
              children: ["Terms of Use"],
            },
            " and the ",
            {
              type: "link",
              href: "/content",
              children: ["Content Policy"],
            },
            ", and does not include gambling or other content restricted to people aged 18 or older under Sections 3 and 5 of the Content Policy.",
          ],
        },
        {
          type: "p",
          content: [
            "5.2 ",
            {
              type: "strong",
              children: ["Qualifying Players."],
            },
            " Only Qualifying Players count toward Rewards. A Qualifying Player is a player session that our systems classify as a genuine human player under the criteria set out on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ".",
          ],
        },
        {
          type: "p",
          content: [
            "5.3 ",
            {
              type: "strong",
              children: ["Excluded activity."],
            },
            " The following do not count toward Rewards: (a) sessions by you, your collaborators or other Program participants; (b) sessions by Excluded Persons; (c) activity generated in breach of Section 8; (d) if a Rewards Program season runs at the same time, visits rewarded under that program; and (e) players brought to your experience through paid advertising campaigns run or funded by the Foundation or DCL Regenesis Labs Foundation, for the metrics and to the extent set out on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ".",
          ],
        },
        {
          type: "p",
          content: [
            "5.4 ",
            {
              type: "strong",
              children: ["Classification is ours."],
            },
            " Classification of sessions and players is carried out by the Foundation and its service providers using signals such as presence data, device and network information and wallet history. We do not disclose detection methods. Our classification is final, subject to Section 8.4 for exclusions of a participant.",
          ],
        },
        {
          type: "p",
          content: [
            "5.5 ",
            {
              type: "strong",
              children: ["Dashboard figures."],
            },
            " Metrics shown in your Creator Hub dashboard are for information only. Rewards are calculated after the review and filtering described in Section 5.4, so the figures used for Rewards may differ from those on your dashboard, and dashboard figures do not by themselves give you a right to any Reward.",
          ],
        },
      ],
    },
    {
      id: "reward-calculation",
      blocks: [
        {
          type: "h2",
          content: ["6. Reward Calculation"],
        },
        {
          type: "p",
          content: [
            "6.1 ",
            {
              type: "strong",
              children: ["Season Pool."],
            },
            ' The amount available for Rewards in each Cycle (the "',
            {
              type: "strong",
              children: ["Cycle Allocation"],
            },
            '") is published on the ',
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ' before that Cycle starts. The Cycle Allocations of a Season together make up the "',
            {
              type: "strong",
              children: ["Season Pool"],
            },
            '". We may increase a Cycle Allocation at any time. We may reduce it only for Cycles that have not yet started, by updating the Program Rules page at least 7 days before that Cycle starts. Total Rewards in a Cycle never exceed its Cycle Allocation. Publishing or changing a Cycle Allocation under this Section is not a change to these Terms under Section 11.1.',
          ],
        },
        {
          type: "p",
          content: [
            "6.2 ",
            {
              type: "strong",
              children: ["How Rewards are calculated."],
            },
            " Each Cycle Allocation is shared among Qualifying Experiences based on the engagement they generate from Qualifying Players. The metrics we use, such as active playtime, player retention and new players, are described on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ". How we weigh, filter and combine them is determined by us and is not published.",
          ],
        },
        {
          type: "p",
          content: [
            "6.3 ",
            {
              type: "strong",
              children: ["Changes to the calculation."],
            },
            " We may change the metrics described on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            " by updating that page before the Cycle in which the change applies. We may adjust weighting and filtering at any time, including to correct errors or prevent abuse. Changes never affect Rewards already calculated.",
          ],
        },
        {
          type: "p",
          content: [
            "6.4 ",
            {
              type: "strong",
              children: ["Our calculation is final."],
            },
            " We calculate Rewards from our own data. You have no right to inspect or audit our data, systems, formulas or records, and we have no obligation to disclose them beyond what these Terms and the Program Rules provide.",
          ],
        },
        {
          type: "p",
          content: [
            "6.5 ",
            {
              type: "strong",
              children: ["Results."],
            },
            " After each Cycle we may publish rankings or leaderboards of Qualifying Experiences under Section 10.4. Your full results for each Cycle are available to you in the Creator Fund Payouts section of Creator Hub.",
          ],
        },
      ],
    },
    {
      id: "payment",
      blocks: [
        {
          type: "h2",
          content: ["7. Payment"],
        },
        {
          type: "p",
          content: [
            "7.1 ",
            {
              type: "strong",
              children: ["Timing."],
            },
            " Subject to Section 4.2, Rewards for a Cycle are paid within 14 days after the Cycle ends, after the checks in Sections 3.4 and 7.6.",
          ],
        },
        {
          type: "p",
          content: [
            "7.2 ",
            {
              type: "strong",
              children: ["Minimum Reward."],
            },
            " A Reward is payable only if it reaches the minimum set out on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            " (currently USD 5). Amounts below the minimum are not earned, are not carried forward and are not paid. Your Reward for a Cycle is the total for all your Qualifying Experiences.",
          ],
        },
        {
          type: "p",
          content: [
            "7.3 ",
            {
              type: "strong",
              children: ["No balances."],
            },
            " Each Cycle is calculated and paid on its own. No balance is carried between Cycles or Seasons, and the Season ends with the payment for its last Cycle.",
          ],
        },
        {
          type: "p",
          content: [
            "7.4 ",
            {
              type: "strong",
              children: ["Currency."],
            },
            " Rewards are calculated in US dollars and paid in USDC as an ERC-20 token on Ethereum mainnet. We may offer other payment currencies, such as MANA, in later Seasons, as set out on the ",
            {
              type: "link",
              href: "https://docs.decentraland.org/creator/creator-fund/program-rules",
              target: "_blank",
              rel: "noopener noreferrer",
              children: ["Program Rules page"],
            },
            ".",
          ],
        },
        {
          type: "p",
          content: [
            "7.5 ",
            {
              type: "strong",
              children: ["Fees."],
            },
            " We pay the network fee for each payment. Any other fees, including exchange or off-ramp fees, are yours.",
          ],
        },
        {
          type: "p",
          content: [
            "7.6 ",
            {
              type: "strong",
              children: ["Sanctions check."],
            },
            " Before each payment we may check the Owner Wallet and the Payout Wallet against public sanctions lists. We will not pay if we know either wallet is listed, and amounts withheld for this reason are not earned.",
          ],
        },
      ],
    },
    {
      id: "fair-play-and-exclusion",
      blocks: [
        {
          type: "h2",
          content: ["8. Fair Play and Exclusion"],
        },
        {
          type: "p",
          content: [
            "8.1 ",
            {
              type: "strong",
              children: ["Fair play."],
            },
            " You must take part in the Program honestly. You must not, directly or through others, do anything that artificially inflates or manipulates your metrics or Rewards, misleads us, or undermines the fairness of the Program for other creators. This includes, for example: (a) generating activity through bots, scripts, fake or multiple accounts, or any other non-genuine means; (b) splitting participation across several LAND parcels, NAMEs, Worlds or wallets, or coordinating with others, to increase Rewards or evade any limit or exclusion; (c) giving false or misleading declarations or information; and (d) misusing the review or dispute process, or engaging in any conduct prohibited by Section 12.3 of the Terms of Use. Promoting your experience or paying or rewarding real players to visit it is not by itself a breach of this Section, and those players count only if they are Qualifying Players.",
          ],
        },
        {
          type: "p",
          content: [
            "8.2 ",
            {
              type: "strong",
              children: ["What we may do."],
            },
            " If we reasonably believe you have engaged in prohibited conduct or breached these Terms, we may: (a) disregard the affected activity; (b) treat your unpaid Rewards as not earned and not pay them; and (c) suspend you while we review the matter, or exclude you from the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "8.3 ",
            {
              type: "strong",
              children: ["Notice."],
            },
            " We will tell you by email when we take action under Section 8.2(b) or (c), with a short reason and how to respond under Section 8.4, unless doing so would breach the law or an authority's instruction, compromise an ongoing investigation or sanctions review, or put the Program or others at risk. We do not disclose our detection methods.",
          ],
        },
        {
          type: "p",
          content: [
            "8.4 ",
            {
              type: "strong",
              children: ["Human review."],
            },
            " No suspension or exclusion of a participant is decided solely by automated means. Each one is reviewed by a member of the Program team before it takes effect. You may send us information within 14 days of the notice, and we will consider it before the decision is final.",
          ],
        },
        {
          type: "p",
          content: [
            "8.5 ",
            {
              type: "strong",
              children: ["Players."],
            },
            " Suspension or termination of any Decentraland account, yours or a player's, is governed by Section 15 of the Terms of Use and is separate from exclusion from the Program. We do not publish the names of players or participants excluded for fraud.",
          ],
        },
        {
          type: "p",
          content: [
            "8.6 ",
            {
              type: "strong",
              children: ["Exclusion."],
            },
            " If we find that you did not cause the affected activity, we only disregard it. Any exclusion under Section 8.2 is permanent. It applies to you as a person or entity and to any wallet, LAND, NAME or World you control or that acts on your behalf, and continues to apply under any later version of these Terms. Taking part through another wallet, LAND, NAME or World during an exclusion is itself a breach of these Terms.",
          ],
        },
      ],
    },
    {
      id: "taxes",
      blocks: [
        {
          type: "h2",
          content: ["9. Taxes"],
        },
        {
          type: "p",
          content: [
            "9.1 You are solely responsible for reporting and paying any taxes, duties or social contributions that apply to Rewards you receive, under the laws of your country.",
          ],
        },
        {
          type: "p",
          content: [
            "9.2 Consistent with Sections 5.8 and 6.5 of the Terms of Use, Rewards are paid without deduction of tax and the Foundation does not act as a withholding agent. If a law that applies to the Foundation nonetheless requires it to collect tax information or withhold tax on Rewards, we will tell you and may hold payment until you provide the information we need. If you do not provide it within 15 days, the Reward is not earned and is not paid.",
          ],
        },
        {
          type: "p",
          content: [
            "9.3 We do not give tax advice. Nothing in the Program or these Terms is a representation about how Rewards are taxed.",
          ],
        },
      ],
    },
    {
      id: "data-and-publicity",
      blocks: [
        {
          type: "h2",
          content: ["10. Data and Publicity"],
        },
        {
          type: "p",
          content: [
            "10.1 ",
            {
              type: "strong",
              children: ["Personal data."],
            },
            " We process the data you give us at opt-in or in Creator Hub (wallet addresses, contact name, email, declarations) and data about your experience and its players, including activity before you opt in under Section 4.6, to run the Program, calculate Rewards, screen payments and prevent fraud, as described in the ",
            {
              type: "link",
              href: "/privacy",
              children: ["Privacy Policy"],
            },
            ". Fraud prevention is carried out on the basis of our legitimate interest in protecting the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "10.2 ",
            {
              type: "strong",
              children: ["Service providers."],
            },
            " We may share this data with entities that help us run the Program, including DCL Regenesis Labs Foundation, analytics providers and sanctions-screening providers, under appropriate data-protection terms.",
          ],
        },
        {
          type: "p",
          content: [
            "10.3 ",
            {
              type: "strong",
              children: ["Retention."],
            },
            " We keep Program data for the Season plus 6 months, or longer where needed to deal with a claim or a legal obligation.",
          ],
        },
        {
          type: "p",
          content: [
            "10.4 ",
            {
              type: "strong",
              children: ["Publicity."],
            },
            " You agree that we may display information about your experience and its results in the Program, such as its scene name, LAND, NAME or World, thumbnail, rank, metrics and the Rewards awarded to it, in Program rankings, leaderboards and Decentraland communications about the Program. We will not publish your contact name, email or wallet addresses. You may ask us to stop using your scene name and thumbnail in new communications at any time; past rankings stay published. Payments are recorded on a public blockchain.",
          ],
        },
        {
          type: "p",
          content: [
            "10.5 ",
            {
              type: "strong",
              children: ["Your content."],
            },
            " You keep all rights in your experience. The Program does not grant us any licence beyond those in the Terms of Use, Section 6 of the Content Policy and Section 10.4.",
          ],
        },
        {
          type: "p",
          content: [
            "10.6 ",
            {
              type: "strong",
              children: ["Emails."],
            },
            " By opting in, you agree to receive emails about the Program, including results, payment confirmations and any notice under Section 8. These emails are part of running the Program and are not marketing.",
          ],
        },
      ],
    },
    {
      id: "changes-suspension-and-termination",
      blocks: [
        {
          type: "h2",
          content: ["11. Changes, Suspension and Termination"],
        },
        {
          type: "p",
          content: [
            "11.1 ",
            {
              type: "strong",
              children: ["Changes to these Terms."],
            },
            " We may change these Terms by posting a new version and notifying you by email at least 14 days before it takes effect. Changes do not affect Rewards already earned. If you do not accept a change, you may leave the Program before it takes effect.",
          ],
        },
        {
          type: "p",
          content: [
            "11.2 ",
            {
              type: "strong",
              children: ["Leaving."],
            },
            " You may leave the Program at any time by contacting us at ",
            {
              type: "link",
              href: "mailto:creatorfund@decentraland.org",
              children: ["creatorfund@decentraland.org"],
            },
            ". Your participation ends on the date we confirm it. Rewards for Cycles completed before that date are paid under Section 7; the Cycle in progress does not count. If you leave during a Season, you may rejoin from the next Season.",
          ],
        },
        {
          type: "p",
          content: [
            "11.3 ",
            {
              type: "strong",
              children: ["Suspension or end of the Program."],
            },
            " We may suspend or end the Program, or, subject to Section 6.1, reduce the Season Pool for future Cycles, at any time. If we do, we will pay all Rewards earned for complete Cycles.",
          ],
        },
        {
          type: "p",
          content: [
            "11.4 ",
            {
              type: "strong",
              children: ["Survival."],
            },
            " Sections 2, 8, 9, 10 and 12 survive the end of your participation or of the Program.",
          ],
        },
      ],
    },
    {
      id: "disclaimers-liability-and-disputes",
      blocks: [
        {
          type: "h2",
          content: ["12. Disclaimers, Liability and Disputes"],
        },
        {
          type: "p",
          content: [
            "12.1 ",
            {
              type: "strong",
              children: ["Disclaimer."],
            },
            ' The Program, its data and its calculations are provided "as is". We do not guarantee that tracking will be uninterrupted or error-free.',
          ],
        },
        {
          type: "p",
          content: [
            "12.2 ",
            {
              type: "strong",
              children: ["Limitation of liability."],
            },
            " To the extent permitted by law, our total liability to you in connection with the Program is limited to the greater of (a) the Rewards earned by you and not yet paid and (b) USD 100, consistent with Section 11.2 of the Terms of Use. We are not liable for indirect or consequential loss, lost profits, or losses caused by blockchain networks, wallets, token prices or third-party services.",
          ],
        },
        {
          type: "p",
          content: [
            "12.3 ",
            {
              type: "strong",
              children: ["Only the Foundation."],
            },
            " The Foundation is the only party responsible to you under these Terms. Consistent with Section 11.5 of the Terms of Use, the Foundation, the DAO and DCL Regenesis Labs Foundation are independent entities, each liable only for its own acts and omissions; the DAO and DCL Regenesis Labs Foundation have no obligations to you under the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "12.4 ",
            {
              type: "strong",
              children: ["Governing law and disputes."],
            },
            " These Terms are governed by the laws of Panama. Any dispute relating to the Program is resolved under Section 18 of the Terms of Use (Binding Arbitration and Class Action Waiver), including its 30-day initial dispute resolution period, ICC arbitration seated in Panama City, and class action waiver. Under Section 19.3 of the Terms of Use, any claim must be brought within one year after it arose.",
          ],
        },
        {
          type: "p",
          content: [
            "12.5 ",
            {
              type: "strong",
              children: ["General."],
            },
            " These Terms are the entire agreement on the Program. You may not transfer your rights under them. If any part is unenforceable, the rest remains in force. Our failure to enforce a right is not a waiver. If Program communications or materials conflict with these Terms, these Terms prevail.",
          ],
        },
        {
          type: "p",
          content: [
            "12.6 ",
            {
              type: "strong",
              children: ["Contact."],
            },
            " Program questions: ",
            {
              type: "link",
              href: "mailto:creatorfund@decentraland.org",
              children: ["creatorfund@decentraland.org"],
            },
            ". Legal notices: ",
            {
              type: "link",
              href: "mailto:legal@decentraland.org",
              children: ["legal@decentraland.org"],
            },
            ".",
          ],
        },
      ],
    },
  ],
};
