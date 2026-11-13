// Ported verbatim from decentraland/sites@cfdfa1a5 (src/pages/content); the text is legal content:
// do not edit it by hand. Its frozen rendering is apps/sites/src/data/legal/fixtures/content.json.
import type { LegalDocument } from "./types";

export const CONTENT: LegalDocument = {
  key: "content",
  slug: "content",
  title: "Content Policy",
  pageTitle: "Content Policy | Decentraland",
  toc: [
    {
      id: "definitions",
      label: "1. Definitions",
    },
    {
      id: "prohibited-content",
      label: "2. Prohibited Content",
    },
    {
      id: "gambling",
      label: "3. Gambling",
    },
    {
      id: "breaches-of-this-policy",
      label: "4. Breaches of this Policy",
    },
    {
      id: "age-restricted-content",
      label: "5. Restricted Content",
    },
    {
      id: "user-representations-and-warranties",
      label: "6. User Representations and Warranties",
    },
    {
      id: "storage",
      label: "7. Storage",
    },
    {
      id: "limitations-to-liability",
      label: "8. Limitations to Liability",
    },
    {
      id: "changes-to-this-policy",
      label: "9. Changes to this Policy",
    },
  ],
  intro: [],
  sections: [
    {
      id: "definitions",
      blocks: [
        {
          type: "h2",
          content: ["1. Definitions"],
        },
        {
          type: "p",
          content: [
            '"Content" shall mean any work of authorship, creative works, graphics, images, textures, photos, logos, video, audio, text and interactive features, including without limitation NFTs, submitted by the Users of Decentraland.',
          ],
        },
        {
          type: "p",
          content: [
            '"Intellectual Property Rights" shall mean rights in, arising out of, or associated with intellectual property in any jurisdiction, including without limitation rights in or arising out of, or associated with (1) copyrights, mask work rights, and other rights in published and unpublished works of authorship, including without limitation computer programs, databases, graphics, user interfaces, and similar works; (2) patents, design rights, and other rights in inventions and discoveries, including without limitation articles of manufacture, business methods, compositions of matter, improvements, machines, methods, and processes; (3) trademarks, service marks, trade dress and other logos and similar indications of origin of, or association with, a group, business, good, product, or service; (4) trade secrets and other information that is not generally known or readily ascertainable by third parties through proper means, whether tangible or intangible, including without limitation algorithms, customer lists, ideas, designs, formulas, know-how, source code, methods, processes, programs, prototypes, systems, and techniques; (5) a person\'s name, voice, signature, photograph, or likeness, including without limitation rights of personality, privacy, and publicity; (6) attribution and integrity and other so-called moral rights of an author; (7) internet domain names; (8) data and databases; and (9) similar proprietary rights arising under the laws of any jurisdiction',
          ],
        },
        {
          type: "p",
          content: [
            '"NFT" means non-fungible token, including, LAND, Wearables and any other kind of NFTs available in Decentraland. All NFTs must comply with this Content Policy in accordance with section 12.4 of the Terms of Use.',
          ],
        },
      ],
    },
    {
      id: "prohibited-content",
      blocks: [
        {
          type: "h2",
          content: ["2. Prohibited Content"],
        },
        {
          type: "p",
          content: [
            "All Content uploaded, posted, created, displayed, transmitted or made available by the User through the Services must not include:",
          ],
        },
        {
          type: "p",
          content: [
            "2.1. Content involving illegality, such as piracy, criminal activity, terrorism, obscenity, child pornography, gambling (subject to Section 3 below), and illegal drug use.",
          ],
        },
        {
          type: "p",
          content: [
            "2.2. Content infringing third party Intellectual Property Rights.",
          ],
        },
        {
          type: "p",
          content: [
            "2.3. Cruel or hateful Content that could harm, harass, promote or condone violence against, or that is primarily intended to incite hatred of, animals, or individuals or groups based on race or ethnic origin, religion, nationality, disability, gender, age, veteran status, or sexual orientation/gender identity.",
          ],
        },
        {
          type: "p",
          content: [
            "2.4. Content that is libelous, false, inaccurate, misleading, or invades another person's privacy.",
          ],
        },
        {
          type: "p",
          content: [
            "2.5. Content that breaches the Privacy Policy or applicable data privacy laws.",
          ],
        },
        {
          type: "p",
          content: [
            "2.6. Content that promotes or could be construed as primarily intended to evade the limitations above.",
          ],
        },
      ],
    },
    {
      id: "gambling",
      blocks: [
        {
          type: "h2",
          content: ["3. Gambling"],
        },
        {
          type: "p",
          content: [
            "If your Content involves gambling, the following shall apply: (i) if you reside in a jurisdiction which requires a license for online gambling, you must obtain such license prior to making your Content available; (ii) you must be in full compliance with the regulations of your country of residence; (iii) you must geo-block your Content for IPs from jurisdictions where online gambling is banned (including, without limitation, the United States of America, South Korea and China) and (iv) you must include in the terms and conditions of use of your Content (if any) a release from liability in favor of the Foundation and the DAO to the fullest extent allowed by applicable law vis a vis you and the users of your Content.",
          ],
        },
      ],
    },
    {
      id: "breaches-of-this-policy",
      blocks: [
        {
          type: "h2",
          content: ["4. Breaches of this Policy"],
        },
        {
          type: "p",
          content: [
            "Any Content in infringement of Section 2, may be blocked and upon blocking shall not be available to other users of the Services. Moreover, infringing Content may result in suspension of the Account, court orders, civil actions, injunctions, criminal prosecutions and other legal consequences brought by the Foundation, the DAO or third parties against the creator, distributor and/or user of said infringing Content. The User's Account may also be terminated in accordance with Section 15 of the Terms of Use.",
          ],
        },
      ],
    },
    {
      id: "age-restricted-content",
      blocks: [
        {
          type: "h2",
          content: ["5. Restricted Content"],
        },
        {
          type: "p",
          content: [
            "Any Content which qualifies as (i) intensely violent or graphic, (ii) gambling or (ii) sexually explicit, shall only be available to people aged 18 or older. If you upload, post, create, display, transmit or make available such Content on the Tools, you must provide sufficient warning as to this restriction. Failure to do so may result in termination of your Account pursuant to Section 15 of the Terms.",
          ],
        },
      ],
    },
    {
      id: "user-representations-and-warranties",
      blocks: [
        {
          type: "h2",
          content: ["6. User Representations and Warranties"],
        },
        {
          type: "p",
          content: [
            "You represent and warrant that at any time you submit Content, you are at least the age of majority in the jurisdiction in which you reside and are the parent or legal guardian, or have all proper consents from the parent or legal guardian, of any minor who contributed to any Content you submit, and that, as to that Content, (a) you are the sole author and owner of the Intellectual Property Rights to such Content, or you have a lawful right to submit the Content, all without any obligation to obtain consent of any third party and without creating any obligation or liability for the Foundation; (b) the Content is accurate; (c) the Content does not and will not infringe any Intellectual Property Right of any third party; and (d) the User Content will not violate the Terms or this Content Policy, or cause injury or harm to any person.",
          ],
        },
        {
          type: "p",
          content: [
            "You expressly acknowledge and accept that the Content you submit will be accessible to and viewable by other users and waive any claim with regards to the Foundation, its directors, officers, employees and affiliates in connection with said third party access. You can withdraw your Content at any time you wish.",
          ],
        },
        {
          type: "p",
          content: [
            "Please remember that the Content that you submit will be accessible to and viewable by other users. Except as may be required to register and/or maintain your Account, do not submit personally identifiable information (e.g. first and last name together, password, phone number, address, credit or debit card number, medical information, e-mail address, or other contact information) on the Tools.",
          ],
        },
        {
          type: "p",
          content: [
            "By submitting, posting or displaying content, and or through Decentraland Platform, the Services and/or the Marketplace, you grant us a worldwide, non-exclusive, royalty-free perpetual, irrevocable, transferable right and license (with the right to sublicense) to use, copy, reproduce, process, adapt, modify, publish, transmit, display, develop, improve, distribute such Content, promote Decentraland, activities, Events, in any and all media or distribution methods (now known or later developed). You further grant Decentraland, the DAO and/or the Foundation, the right to use your name and trademarks, if any, in connection with our use of your Content available at the Platform and/or the Marketplace, from time to time.",
          ],
        },
      ],
    },
    {
      id: "storage",
      blocks: [
        {
          type: "h2",
          content: ["7. Storage"],
        },
        {
          type: "p",
          content: [
            'You acknowledge that due to the decentralized nature of Decentraland and of the blockchain technology, all Content and information submitted by you is not stored in a centralized server, but in several decentralized nodes (the "Nodes"). Thus, the Foundation or the DAO are not liable for any loss of content or information.',
          ],
        },
        {
          type: "p",
          content: [
            "The Nodes recognize and accept that in the event of any court order relating to the blocking, suspension or deletion of any Content, they will abide by any such court order.",
          ],
        },
      ],
    },
    {
      id: "limitations-to-liability",
      blocks: [
        {
          type: "h2",
          content: ["8. Limitations to Liability"],
        },
        {
          type: "p",
          content: [
            "The Foundation, its officers, employees, and the DAO are not responsible or liable for the Content, conduct, or services of users or third parties. The Foundation, its officers, employees and the DAO do not control or endorse the Content of communications between users or users' interactions with each other or the Tools.",
          ],
        },
        {
          type: "p",
          content: [
            "You acknowledge that you will be exposed to various aspects of the Services involving the conduct, Content, and services of users, and that the Foundation does not control and is not responsible or liable for the quality, safety, legality, truthfulness or accuracy of any such user conduct, Content or user services. You acknowledge that Decentraland does not guarantee the accuracy of information submitted by any user of the Services, nor any identity information about any user. Your interactions with other users and your use of Content are entirely at your own risk. The Foundation has no obligation to become involved in any dispute that you may have or claim to have with one or more users of the Services, or in any manner in any resolution thereof.",
          ],
        },
        {
          type: "p",
          content: [
            "THE TOOLS MAY CONTAIN LINKS TO OR OTHERWISE ALLOW CONNECTIONS TO THIRD-PARTY WEBSITES, SERVERS, AND ONLINE SERVICES OR ENVIRONMENTS THAT ARE NOT OWNED OR CONTROLLED BY THE FOUNDATION. DECENTRALAND, ITS OFFICERS, EMPLOYEES AND THE DAO ARE NOT RESPONSIBLE OR LIABLE FOR THE CONTENT, POLICIES OR PRACTICES OF ANY THIRD-PARTY WEBSITES, SERVERS OR ONLINE SERVICES OR ENVIRONMENTS. Please consult any applicable terms of use and privacy policies provided by the third party for such websites, servers or online services or environments.",
          ],
        },
        {
          type: "p",
          content: [
            'You acknowledge that the Content of the Services is provided or made available to you under license from independent Content Providers, including other users of the Services ("Content Providers"). You acknowledge and agree that except as expressly provided in this Agreement, the Intellectual Property Rights of other Content Providers in their respective Content are not licensed to you by your mere use of the Services. You must obtain from the applicable Content Providers any necessary license rights in Content that you desire to use or access.',
          ],
        },
        {
          type: "p",
          content: [
            "You copy and use Content at your own risk. You are solely responsible and liable for your use, reproduction, distribution, modification, display, or performance of any Content in violation of any Intellectual Property Rights. You agree that Decentraland will have no liability for, and you agree to defend, indemnify, and hold Decentraland harmless from, any claims, losses or damages arising out of or in connection with your use, reproduction, distribution, modification, display, or performance of any Content.",
          ],
        },
      ],
    },
    {
      id: "changes-to-this-policy",
      blocks: [
        {
          type: "h2",
          content: ["9. Changes to this Policy"],
        },
        {
          type: "p",
          content: [
            "The Foundation and/or the DAO may change this Content Policy from time to time. All users have the obligation to be aware of the updated versions of this Policy.",
          ],
        },
      ],
    },
  ],
};
