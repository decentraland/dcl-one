// Ported verbatim from decentraland/sites@cfdfa1a5 (src/pages/credits-terms); the text is legal content:
// do not edit it by hand. Its frozen rendering is apps/sites/src/data/legal/fixtures/credits-terms.json.
import type { LegalDocument } from "./types";

export const CREDITS: LegalDocument = {
  key: "credits",
  slug: "credits-terms",
  title: "Credits",
  pageTitle: "Decentraland",
  toc: [
    {
      id: "general-provisions",
      label: "1. General Provisions",
    },
    {
      id: "credits",
      label: "2. Credits",
    },
    {
      id: "eligibility",
      label: "3. Eligibility",
    },
    {
      id: "purchase-of-credits",
      label: "4. Purchase of Credits",
    },
    {
      id: "use-of-credits",
      label: "5. Use of Credits",
    },
    {
      id: "validity",
      label: "6. Validity",
    },
    {
      id: "non-transferability",
      label: "7. Non-Transferability and Restrictions",
    },
    {
      id: "refunds",
      label: "8. Refunds",
    },
    {
      id: "miscellaneous",
      label: "9. Miscellaneous",
    },
    {
      id: "modifications",
      label: "10. Modifications to These Terms",
    },
  ],
  intro: [
    {
      type: "p",
      content: ["Terms and Conditions"],
    },
    {
      type: "p",
      content: ["Last Updated: August 4, 2026"],
    },
    {
      type: "p",
      content: ["Previous Update: June 18, 2026"],
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
            '1.1. These Terms and Conditions (the "Credits Terms") govern the purchase and use of Decentraland Credits ("Credits") within the Decentraland iOS mobile application, the Decentraland platform and any future mobile or web application, if applicable.',
          ],
        },
        {
          type: "p",
          content: [
            '1.2. Credits are offered by the Decentraland Foundation ("Foundation", "we", "us", or "our").',
          ],
        },
        {
          type: "p",
          content: [
            "1.3. Purchasing Credits constitutes acceptance of these Terms, as well as the Decentraland Terms of Use, Privacy Policy, and Content Policy.",
          ],
        },
        {
          type: "p",
          content: [
            "1.4. The Foundation reserves the right to modify, suspend, or terminate the availability of Credits, or the functionality through which Credits may be acquired or used at its sole discretion, subject to the notice provisions set out in Section 10.",
          ],
        },
      ],
    },
    {
      id: "credits",
      blocks: [
        {
          type: "h2",
          content: ["2. Credits"],
        },
        {
          type: "p",
          content: [
            '2.1. Credits are a limited digital in-app utility purchased with real money via Apple In-App Purchase ("IAP") on the Decentraland iOS mobile application, via Stripe on the Decentraland platform or any other online payment service provider and/or app to be connected to Decentraland apps or platform in the future.',
          ],
        },
        {
          type: "p",
          content: [
            "2.2. Credits are available solely for use within Decentraland. Credits do not constitute money, stored value, cryptocurrency, electronic money, a payment instrument, or any form of financial asset. Credits are not redeemable, refundable, exchangeable, or transferable for real currency or MANA, except as expressly provided in these Terms.",
          ],
        },
        {
          type: "p",
          content: [
            "2.3. Credits are licensed, not sold. Users obtain a limited, revocable, non-exclusive, non-transferable right to use Credits solely within the Decentraland ecosystem in accordance with these Terms.",
          ],
        },
        {
          type: "p",
          content: [
            "2.4. Where Credits are purchased via IAP, these Credits are licensed content provided by the Foundation and are not sold by Apple. Apple has no obligation to provide maintenance, support, refunds, or warranties relating to Credits except as required under Apple's applicable policies.",
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
          content: ["3.1. To purchase Credits, you must:"],
        },
        {
          type: "ul",
          items: [
            ["hold a fully registered Decentraland account;"],
            [
              "be at least 18 years of age (or the age of majority in your jurisdiction, whichever is higher); and",
            ],
            [
              "not to be located in, ordinarily resident in, or otherwise subject to the laws of any jurisdiction where the offering, purchase, or use of Credits is prohibited, including, without limitation, the United Kingdom.",
            ],
          ],
        },
        {
          type: "p",
          content: [
            "3.2. The Foundation reserves the right to restrict access to Credits in additional jurisdictions at its sole discretion.",
          ],
        },
      ],
    },
    {
      id: "purchase-of-credits",
      blocks: [
        {
          type: "h2",
          content: ["4. Purchase of Credits"],
        },
        {
          type: "p",
          content: [
            "4.1. Credits may only be purchased through (a) IAP on iOS devices, (b) Stripe on the Decentraland platform, or (c) any other online payment service provider and/or app to be connected to Decentraland apps or platform in the future. The purchase of Credits is processed through Apple's, Stripe's or any other online service providers checkout flow, as applicable, which constitutes explicit consent to the transaction.",
          ],
        },
        {
          type: "p",
          content: [
            "4.2. All purchases are subject to the daily purchase cap and holding cap limits set by the Foundation from time to time and to Apple's applicable App Store policies or, where applicable, Stripe's or other online payment service providers, applicable terms of service. The Foundation may modify such limits at any time.",
          ],
        },
        {
          type: "p",
          content: [
            "4.3. When you use Credits to acquire a Wearable, Emote, or any other item of digital content, you expressly request immediate delivery and performance of that digital content at the moment of transaction. By confirming the acquisition, you consent, acknowledge and agree that:",
          ],
        },
        {
          type: "ul",
          items: [
            [
              "the digital content is delivered to your self-custodial wallet immediately upon confirmation;",
            ],
            [
              "because performance of the digital content supply has begun at your express request before the expiry of any applicable withdrawal period, you waive your right of withdrawal in respect of that acquisition, to the fullest extent permitted by applicable law; and",
            ],
            [
              "Credits used to complete the acquisition are consumed, final, and non-refundable by the Foundation.",
            ],
          ],
        },
        {
          type: "p",
          content: [
            "4.4. The price of Credits is displayed in the application at the time of purchase. The Foundation reserves the right to change Credit pricing at any time.",
          ],
        },
        {
          type: "p",
          content: [
            "4.5. All payments are processed by Apple, Stripe or any future online payment service provider, as applicable. The Foundation does not store or process your payment information directly.",
          ],
        },
        {
          type: "p",
          content: [
            "4.6. You accept that any applicable taxes in connection with the purchase of Credits shall be paid by you directly. You are solely responsible for any taxes, duties, or any charges arising from the acquisition, use, or disposition of Credits and any digital content acquired with Credits.",
          ],
        },
        {
          type: "p",
          content: [
            "4.7. The Foundation reserves the right to correct pricing, display, or technical errors relating to Credits purchases and may cancel or adjust transactions affected by such errors.",
          ],
        },
        {
          type: "p",
          content: [
            "4.8. The Foundation may restrict, suspend, or prohibit purchases of Credits where required to comply with sanctions laws, anti-money laundering obligations, fraud prevention measures, or other legal requirements.",
          ],
        },
      ],
    },
    {
      id: "use-of-credits",
      blocks: [
        {
          type: "h2",
          content: ["5. Use of Credits"],
        },
        {
          type: "p",
          content: [
            "5.1. Credits may only be used on primary sales of Polygon Wearables & Emotes. The Foundation may expand, restrict or modify the permitted uses of Credits at any time, subject to the applicable notice requirements.",
          ],
        },
        {
          type: "p",
          content: [
            "5.2. Credits may also be used for scene and content submission fees within the Decentraland Builder, where applicable.",
          ],
        },
        {
          type: "p",
          content: ["5.3. Credits do not represent voting power (VP)."],
        },
        {
          type: "p",
          content: [
            "5.4. Credits cannot be exchanged for cash, MANA, or any other monetary or alternative benefit.",
          ],
        },
        {
          type: "p",
          content: [
            "5.5. Credits cannot be used to publish a collection, used on Ethereum mainnet, or transferred between users.",
          ],
        },
        {
          type: "p",
          content: [
            "5.6. The availability, pricing, and eligibility of Wearables, Emotes, and other digital content purchasable with Credits may change at any time.",
          ],
        },
      ],
    },
    {
      id: "validity",
      blocks: [
        {
          type: "h2",
          content: ["6. Validity"],
        },
        {
          type: "p",
          content: [
            "6.1. Credits currently do not expire. However, the Foundation reserves the right to set expiration dates or discontinue the Credits in the future providing at least 30 days prior notice through any of the available communication channels, including, but not limited to email addresses associated with your account and/or by in-app notification, etc.",
          ],
        },
        {
          type: "p",
          content: [
            "6.2. The Foundation shall have no obligation to provide compensation, refunds or cash equivalents in connection with any expiration or discontinuation of Credits.",
          ],
        },
      ],
    },
    {
      id: "non-transferability",
      blocks: [
        {
          type: "h2",
          content: ["7. Non-Transferability and Restrictions"],
        },
        {
          type: "p",
          content: [
            "7.1. Credits are non-transferable and may only be used by the account holder.",
          ],
        },
        {
          type: "p",
          content: [
            "7.2. Credits must be held by a specific account (linked to a valid EVM address) and can only be utilized by that account.",
          ],
        },
        {
          type: "p",
          content: [
            "7.3. The Foundation reserves the right to revoke or cancel Credits if they are misused or if a participant is found to be in violation of these Terms or the Decentraland Terms of Use. If your account is suspended, terminated, or otherwise modified, or if your right to access the Decentraland platform is terminated for any reason, you may lose access to some or all of your Credits. The Foundation has no obligation to restore Credits lost as a result of account suspension or termination.",
          ],
        },
        {
          type: "p",
          content: [
            "7.4. Acquisition of a Wearable, Emote, or other digital content using Credits does not transfer any intellectual property rights except for the limited rights expressly granted under the applicable Decentraland Terms of Use.",
          ],
        },
        {
          type: "p",
          content: [
            "7.5. The Foundation is not responsible for any loss of Credits, Wearables, Emotes, or other digital content resulting from loss of access to a wallet, compromise of credentials, blockchain events, or actions taken by third-party wallet providers.",
          ],
        },
      ],
    },
    {
      id: "refunds",
      blocks: [
        {
          type: "h2",
          content: ["8. Refunds"],
        },
        {
          type: "p",
          content: [
            "8.1. All purchases of Credits through IAP are subject to Apple's refund policies. Refund requests for the IAP purchase must be directed to Apple. Purchases of Credits made via Stripe or any future online payment service provider, are final and non-refundable, except as required by applicable law. The Foundation does not process refunds for Credit purchases directly.",
          ],
        },
        {
          type: "p",
          content: [
            "8.2. When you use Credits to acquire a Wearable, Emote, or any other digital content, the Credits are immediately deducted from your balance and the transaction is final, non-refundable, and non-reversible. This applies regardless of any refund, chargeback, or reversal decision on the original purchase whether made via IAP, Stripe or any future online payment service provider.",
          ],
        },
        {
          type: "p",
          content: [
            "8.3. If Apple, Stripe or any online payment service provider grants a refund, chargeback, reversal, or other payment recovery after Credits have been used, the Foundation may deduct Credits, revoke access to associated digital content, suspend access to certain features, or take any other reasonable action necessary to address the resulting negative balance.",
          ],
        },
      ],
    },
    {
      id: "miscellaneous",
      blocks: [
        {
          type: "h2",
          content: ["9. Miscellaneous"],
        },
        {
          type: "p",
          content: [
            '9.1. The Foundation is not responsible for any technical malfunctions, errors, or issues that may affect the availability of Credits, to the maximum extent permitted by applicable law. Credits are provided on an "as is" and "as available" basis without warranties of any kind.',
          ],
        },
        {
          type: "p",
          content: [
            "9.2. The Foundation may investigate any suspected fraudulent, abusive, or unlawful activity relating to the acquisition or use of Credits and may suspend, limit, revoke, or cancel Credits pending such investigation. The Foundation may request additional information or documentation from users in connection with such investigations.",
          ],
        },
        {
          type: "p",
          content: [
            "9.3. If Credits are discontinued, the Foundation may provide a reasonable redemption period during which Users may use their remaining Credits before they are cancelled.",
          ],
        },
        {
          type: "p",
          content: [
            "9.4. Any disputes regarding these Credits Terms shall be resolved in accordance with the dispute resolution provisions of the Decentraland Terms of Use.",
          ],
        },
        {
          type: "p",
          content: [
            "9.5. Apple and Apple's subsidiaries are third-party beneficiaries of these Credits Terms and may enforce these Credits Terms and, upon your acceptance of these Terms, shall have the right (and shall be deemed to have accepted the right) to enforce these Terms against you as third party beneficiary thereof.",
          ],
        },
        {
          type: "p",
          content: [
            "9.6. Credits do not represent any ownership interest, equity, or claim against the Foundation or the Decentraland DAO.",
          ],
        },
        {
          type: "p",
          content: [
            "For further inquiries, please contact ",
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
    {
      id: "modifications",
      blocks: [
        {
          type: "h2",
          content: ["10. Modifications to These Terms"],
        },
        {
          type: "p",
          content: [
            "10.1. The Foundation may update these Credits Terms at any time. For changes that materially affect your rights, the Foundation will provide no less than 30 days prior written notice through any of the available communication channels, including, but not limited to: email address associated with your account and/or by in-app notification, etc.",
          ],
        },
        {
          type: "p",
          content: [
            "10.2. By purchasing Credits, you consent to receive transactional and service-related communications regarding your Credits balance and purchases.",
          ],
        },
        {
          type: "p",
          content: [
            "10.3. Your continued use of Credits following the effective date of any update constitutes your acceptance of the revised Credits Terms.",
          ],
        },
      ],
    },
  ],
};
