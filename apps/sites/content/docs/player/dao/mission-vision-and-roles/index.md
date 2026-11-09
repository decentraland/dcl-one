> >-

# Mission, Vision and Roles


> [!NOTE]
> **Status:** Official · **Version:** 1.0 · **Last updated:** October 2026 · **Source:** [A shared mission for the Decentraland DAO](https://decentraland.org/governance/proposal/?id=923d0c82-7ba7-4c39-836a-17f959741b1a), passed by the community on November 7, 2024 (Yes 98%, 13.06M VP).


## Vision

> **Own your digital life through open standards to enable social connections.**

The vision was developed at the Regenesis Summit (September 2024), whose participants included people from the Decentraland Foundation, and is meant to be ever-lasting and shared across the whole ecosystem: the community, the DAO, DCL Regenesis Labs and the Decentraland Foundation. No single organization is responsible for achieving it on its own.

## Mission of the Decentraland DAO

> **Ensure the sustainability of the Decentraland Ecosystem through representative governance, efficient management of collective resources, and technical innovation.**

The mission focuses the DAO on the long-term sustainability of the ecosystem. It rests on three pillars:

* **Representative governance.** The DAO reflects the community's voice and aligns with Decentraland's goals while keeping decision-making efficient.
* **Collective resources.** The DAO supports creating, curating and discovering content, and manages the Treasury to secure the project's future.
* **Technical innovation.** The DAO keeps Decentraland at the forefront of metaverse technology, promoting decentralization and open standards.

### Responsibilities of the DAO

1. **Fund strategic projects that improve the experience of visitors, creators, and developers.** Resources go to projects that enhance the in-world experience, streamline creation tools and lower barriers for developers, while respecting openness, decentralization and privacy.
2. **Safeguard the critical smart contracts of the project.** The DAO ensures the security and integrity of the key contracts that make Decentraland work.
3. **Manage the Treasury responsibly to extend the runway of the project.** The Treasury is managed for long-term financial sustainability.
4. **Enable stakeholders to participate in decision-making about the project's future.** The DAO creates mechanisms for community members to shape the direction of Decentraland.

## Who does what

Decentraland is run by several bodies with different mandates. This is the map.

<table><thead><tr><th width="190">Body</th><th>Responsible for</th><th width="220">Not responsible for</th></tr></thead><tbody><tr><td><strong>Decentraland DAO</strong><br>VP holders: MANA, LAND, Estate, NAME and L1 Wearable holders, and their delegates</td><td>The governance process (polls, drafts and binding Governance proposals); ownership of the Treasury and the <a href="/docs/player/dao/what-smart-contracts-does-the-dao-control/">critical smart contracts</a>; electing and dismissing the DAO Council; vetoing Council decisions (such as the annual strategy and budget, the treasury management strategy, or the hiring or firing of the Executive Director) through a Veto Proposal filed within 14 days of the decision being published; delegating voting power.</td><td>Day-to-day operations, the desktop client, Decentraland's IP and domains.</td></tr><tr><td><strong>DAO Council</strong><br>5 members elected by VP holders</td><td>Hiring, guiding and if necessary replacing the Executive Director; approving and publishing the annual strategy and budget; approving the treasury management strategy; formal quarterly reviews of the Executive Arm; publishing reports; appointing, rotating and replacing the signers of the <a href="#multisigs">DAO Operational Multisig</a>; enacting passed proposals: the Council marks them as enacted in the Governance dApp once the on-chain change is made (by the DAO Operational Multisig, by a Security Advisory Board vote for transfers from the Aragon Agent and contract upgrades, or through the DAO's Aragon contracts for the Points of Interest, Catalyst and banned-name lists); updating proposal statuses in the Governance dApp (resolving proposals that end with custom options as passed or rejected; a finished multiple-choice Pre-proposal Poll must be marked as passed before it can be promoted to the next stage by anyone who meets the submission threshold).</td><td>Executing initiatives itself; changing governance rules without a community vote.</td></tr><tr><td><strong>DCL Regenesis Labs</strong><br>The DAO's Executive Arm</td><td>Annual planning and budgeting aligned with the vision and mission; strategic initiatives in three areas: Governance &#x26; Treasury, Ecosystem Growth and Protocol Development (alternative clients, SDK tools, infrastructure, public goods); implementing, with third-party providers, the treasury management strategy approved by the DAO Council; running community funding programs such as the Regenesis Grants Program; maintaining the governance platform and transparency tooling; quarterly transparency reports; answering the quarterly RFIs run by the DAO Council; All Hands.</td><td>Modifying governance mechanisms; ownership of the Treasury or the smart contracts (both belong to the DAO). As the maintainer of the Governance dApp, DCL Regenesis Labs holds a wallet with permission to update proposal statuses.</td></tr><tr><td><strong>Decentraland Foundation</strong><br>Independent non-profit (2020)</td><td>Protecting Decentraland's intellectual property, trademarks, domains, open-source repositories and social accounts on behalf of the ecosystem; promoting the protocol through marketing, events and partnerships; maintaining the desktop client and the project's critical infrastructure services (decentraland.org, the Marketplace app, documentation); enabling creators by building creation tools.</td><td>The DAO's governance process; decisions on the DAO Treasury; deciding DAO policy.</td></tr><tr><td><strong>Security Advisory Board</strong></td><td>Guarantor of smart-contract security: can pause, resume or cancel actions queued in the DAO's Aragon contracts during their 24-hour delay; responds to vulnerability reports in Decentraland's contracts (bug bounty reports are triaged by the Foundation); safeguards the DAO's critical contracts: upgrades to the LAND and Estate contracts are made through an SAB vote, which passes only if at least three of its five members vote in favor and none vote against; the SAB also executes transfers from the DAO's Aragon Agent treasury wallet.</td><td>Policy or funding decisions.</td></tr><tr><td><strong>Wearables Curation Committee</strong></td><td>Reviewing and approving Wearables and Emotes before they are published. Under Governance proposal DAO:ad37b53 (passed July 28, 2026), technical checks are run by an open-source automated validator maintained by DCL Regenesis Labs, and a three-member Committee reviews flagged items, supports creators and proposes validator rules.</td><td>Anything outside curation.</td></tr></tbody></table>

### How the pieces fit together

* The **community** sets policy and elects the Council. Governance proposal: [Proposal for Establishing a Council for Decentraland DAO](https://forum.decentraland.org/t/dao-e1ff3d8-proposal-for-establishing-a-council-for-decentraland-dao/23282) (passed November 14, 2024).
* The **Council** directs and oversees the Executive Arm. Governance proposal: [Establishing an Executive Arm to improve DAO Operations](https://forum.decentraland.org/t/dao-65bfb49-establishing-an-executive-arm-to-improve-dao-operations/23858) (passed March 10, 2025).
* **DCL Regenesis Labs** executes and reports. Updates: [DCL Regenesis Labs on the Forum](https://forum.decentraland.org/c/regenesis-labs/18) · [Financial transparency](https://transparency.dclregenesislabs.xyz/) · [Quarterly RFIs](https://forum.decentraland.org/c/rfi/16).
* The **Decentraland Foundation** protects Decentraland's intellectual property, promotes the protocol through marketing, maintains the desktop client and the project's critical infrastructure services, and enables creators by building tools.

### Multisigs

The DAO's funds are held by its Aragon Agent, which only moves them when an on-chain vote passes (in practice, a Security Advisory Board vote that carries out a passed Governance proposal), and by these Safe multisigs. Every signer and every transaction can be checked on-chain.

| Multisig | Network and address | Threshold | Signers | Used for |
|---|---|---|---|---|
| DAO Operational Multisig | Ethereum [`0x89214c8Ca9A49E60a3bfa8e00544F384C93719b1`](https://app.safe.global/home?safe=eth:0x89214c8Ca9A49E60a3bfa8e00544F384C93719b1)<br>Polygon [`0xB08E3e7cc815213304d884C88cA476ebC50EaAB2`](https://app.safe.global/home?safe=matic:0xB08E3e7cc815213304d884C88cA476ebC50EaAB2) | 3 of 5 | Two DAO Council members, one Security Advisory Board member, one Decentraland Foundation representative and one security specialist or technically capable representative from the Decentraland ecosystem, all appointed by the DAO Council ([DAO:a005416](https://decentraland.org/governance/proposal/?id=bb2b8234-42aa-4ca2-a049-3c7355d4caa4)) | Executing passed Governance proposals and Council-approved mandates on-chain. It has no policy-making power. |
| DAO Council Operational Multisig | Ethereum [`0x184e4D9A26Add0aF1eAfC145550E890a421f16d7`](https://app.safe.global/home?safe=eth:0x184e4D9A26Add0aF1eAfC145550E890a421f16d7) | 3 of 5 | The five DAO Council members | The Council's monthly stipends, payments to the Wearables curators and related operating expenses |
| DAO Treasury Mandate Multisig | Ethereum [`0xC3bfC8C27E69ADE0B852dE1524ae1E9D2019303c`](https://app.safe.global/home?safe=eth:0xC3bfC8C27E69ADE0B852dE1524ae1E9D2019303c) | 3 of 5 | The five DAO Council members | Holding the DAO Treasury assets under the treasury management strategy approved by the DAO Council |

## Where this is published

The DAO committed to displaying its mission prominently in all DAO communication channels and governance interfaces. It is published in:

* [Governance dApp](https://decentraland.org/governance/transparency/) (Transparency page and home)
* [DCL Regenesis Labs financial transparency site](https://transparency.dclregenesislabs.xyz/)
* This documentation page

If you find a copy that differs from this page, this page wins. Report it in the [Governance category](https://forum.decentraland.org/c/governance/13) of the Forum.

## Changelog

| Version | Date | Change |
|---|---|---|
| 1.0 | October 2026 | First publication of the mission approved in November 2024 (DAO:c9e5d8a), the shared vision it references, and the current roles of each body. |
