// Ported verbatim from decentraland/sites@cfdfa1a5 (src/pages/referral-terms); the text is legal content:
// do not edit it by hand. Its frozen rendering is apps/sites/src/data/legal/fixtures/referral-terms.json.
import type { LegalDocument } from "./types";

export const REFERRAL: LegalDocument = {
  key: "referral",
  slug: "referral-terms",
  title: "Referral",
  pageTitle: "Referral Program Terms | Decentraland",
  toc: [
    {
      id: "general-provisions",
      label: "1. General Provisions",
    },
    {
      id: "how-it-works",
      label: "2. How It Works",
    },
    {
      id: "rewards-structure",
      label: "3. Rewards Structure",
    },
    {
      id: "reward-distribution",
      label: "4. Reward Distribution and Delivery",
    },
    {
      id: "program-abuse",
      label: "5. Program Abuse and Disqualification",
    },
    {
      id: "email-notifications",
      label: "6. Email Notifications",
    },
    {
      id: "changes-and-termination",
      label: "7. Changes and Termination",
    },
    {
      id: "miscellaneous",
      label: "8. Miscellaneous",
    },
  ],
  intro: [
    {
      type: "p",
      content: ["Referral Program – Terms and Conditions"],
    },
  ],
  sections: [
    {
      id: "general-provisions",
      blocks: [
        {
          type: "h2",
          content: ["1. General Provisions"],
        },
        {
          type: "p",
          content: [
            '1.1. This Referral Program ("Program") is open to all registered users of Decentraland platform who are in good standing and comply with our general Terms of Service.',
          ],
        },
        {
          type: "p",
          content: [
            "1.2. Participation in the Program constitutes acceptance of these Terms and Conditions, as well as the Decentraland Terms of Use, Privacy Policy, and Content Policy.",
          ],
        },
        {
          type: "p",
          content: [
            "1.3. The Decentraland Foundation reserves the right to modify, suspend, or terminate the Program at its discretion.",
          ],
        },
      ],
    },
    {
      id: "how-it-works",
      blocks: [
        {
          type: "h2",
          content: ["2. How It Works"],
        },
        {
          type: "p",
          content: [
            '2.1. To refer a new user, share your unique referral link. A referred user ("Referee") will be considered valid if the following conditions are met:',
          ],
        },
        {
          type: "ul",
          items: [
            [
              "The Referee successfully creates an account on our platform using a referral link.",
            ],
            [
              "The Referee downloads and installs Decentraland's desktop client.",
            ],
            [
              "The Referee jumps into Genesis City via the desktop client three (3) different days.",
            ],
          ],
        },
        {
          type: "p",
          content: [
            "2.2. Only referrals meeting those criteria will count towards your reward tally.",
          ],
        },
        {
          type: "p",
          content: [
            '2.3. Every successful referral should count and be displayed in a counter under the "Referral Rewards" section.',
          ],
        },
      ],
    },
    {
      id: "rewards-structure",
      blocks: [
        {
          type: "h2",
          content: ["3. Rewards Structure"],
        },
        {
          type: "p",
          content: [
            'Rewards are granted based on the number of valid referrals you accumulate in the "Referral Rewards" section. Each reward tier is cumulative and unlocks sequentially:',
          ],
        },
        {
          type: "ul",
          items: [
            [
              "5 Friends Joined: EPIC Bottoms Wearable + Starter Community Recruiter Badge",
            ],
            [
              "10 Friends Joined: EPIC Jacket Wearable + Bronze Community Recruiter Badge",
            ],
            ["20 Friends Joined: LEGENDARY Handwear Wearable"],
            [
              "25 Friends Joined: LEGENDARY Emote + Silver Community Recruiter Badge",
            ],
            ["30 Friends Joined: EXOTIC Shoes Wearable"],
            [
              "50 Friends Joined: EXOTIC Hair Wearable + Gold Community Recruiter Badge",
            ],
            ['60 Friends Joined: MYTHIC "Companion-style" Wearable'],
            [
              "75 Friends Joined: MYTHIC Looping Emote+ Platinum Community Recruiter Badge",
            ],
            [
              "100 Friends Joined: IRL Swag Pack + Digital Twin Wearable + Diamond Community Recruiter Badge",
            ],
          ],
        },
      ],
    },
    {
      id: "reward-distribution",
      blocks: [
        {
          type: "h2",
          content: ["4. Reward Distribution and Delivery"],
        },
        {
          type: "p",
          content: [
            "4.1. Digital rewards (Wearables, Emotes, badges, and Credits) will be automatically credited to your account within 3 business days of reaching a reward tier.",
          ],
        },
        {
          type: "p",
          content: [
            "4.2. IRL rewards require contact through the designated email or the email address you share for such purpose.",
          ],
        },
        {
          type: "p",
          content: [
            "4.3. We commit to delivering the Rewards in accordance with the rarity levels of the Wearables as stated in the Program. However, we do not guarantee the delivery of a specific Wearable within a given rarity category, as each Wearable is subject to availability and are traded on the Marketplace.",
          ],
        },
      ],
    },
    {
      id: "program-abuse",
      blocks: [
        {
          type: "h2",
          content: ["5. Program Abuse and Disqualification"],
        },
        {
          type: "p",
          content: [
            "We reserve the right to disqualify participants who attempt to game, abuse, or manipulate the referral system, including but not limited to creating fake accounts, using bots, or violating the platform's Terms of Service.",
          ],
        },
      ],
    },
    {
      id: "email-notifications",
      blocks: [
        {
          type: "h2",
          content: ["6. Email Notifications"],
        },
        {
          type: "p",
          content: [
            '6.1. By participating in the Program, you consent to receive email notifications related to "Referral Program."',
          ],
        },
        {
          type: "p",
          content: [
            "6.2. The Decentraland Foundation may add additional email notification categories in the future.",
          ],
        },
      ],
    },
    {
      id: "changes-and-termination",
      blocks: [
        {
          type: "h2",
          content: ["7. Changes and Termination"],
        },
        {
          type: "p",
          content: [
            "We reserve the right to modify or terminate this Program at any time without prior notice. Any changes will be effective immediately upon posting an updated version of these Terms and Conditions.",
          ],
        },
      ],
    },
    {
      id: "miscellaneous",
      blocks: [
        {
          type: "h2",
          content: ["8. Miscellaneous"],
        },
        {
          type: "p",
          content: [
            "7.1. The Decentraland Foundation is not responsible for any technical malfunctions, errors, or issues that may affect participation in the Program.",
          ],
        },
        {
          type: "p",
          content: [
            "7.2. Any disputes regarding the Program shall be resolved at the sole discretion of the Decentraland Foundation.",
          ],
        },
        {
          type: "p",
          content: [
            "7.3. For any questions regarding the Referral Program, please contact our support team via the platform or at ",
            {
              type: "link",
              href: "mailto:support@decentraland.org",
              children: ["support@decentraland.org"],
            },
            ".",
          ],
        },
        {
          type: "p",
          content: ["Updated August 18, 2025"],
        },
      ],
    },
  ],
};
