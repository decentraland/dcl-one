# Windows event poster verification

On November 24, 2026 the stock Windows player showed ten blank calendar cards out of twelve, an empty SkeleFrenz detail cover and six blank Chiri Upcoming Events thumbnails. With the CI-built candidate gateway, each card, each of the six thumbnails and both inspected detail covers displayed its source artwork on the first visit and on revisit. [Visual comparison](comparison.png)

| Surface | Stock player, public gateway | Local candidate, first visit | Local candidate, revisit |
| --- | --- | --- | --- |
| November 28 calendar | [Calendar](baseline-calendar.png) | [Calendar](first-calendar.png) | [Calendar](revisit-calendar.png) |
| SkeleFrenz Gamenight | [Cover](baseline-skelefrenz.png) | [Cover](first-skelefrenz.png) | [Cover](revisit-skelefrenz.png) |
| Chiri Upcoming Events | [Six rows](baseline-chiri.png) | [Six rows](first-chiri.png) | [Six rows](revisit-chiri.png) |
| Community Building PNG | [Calendar card](baseline-png-control.png) | [Detail cover](first-png-control.png) | [Detail cover](revisit-png-control.png) |

The native screen captures were cropped to the relevant panel. The manifest gives pixel coordinates, a hash for the original screen and a hash for the published crop. The artwork comparison uses the event API's published image and checks the expected image against the other visible sources. All expected images ranked first on both visits. [Screen captures](screenshot-manifest.json) · [Baseline](baseline-artwork-matches.json) · [First visit](first-artwork-matches.json) · [Revisit](revisit-artwork-matches.json)

The stock baseline used Windows CI build 28116. Build 28128 adds a QA-only switch for sending unsigned public poster requests to the local candidate. The UI, sign-in and image decoder use baseline code. The production change is in Catalyst, at `1a3f8223380c1ee44e26397862fde0a61db5c0bd`. [Stock CI artifact identity](stock-player.json) · [QA CI artifact identity](qa-player.json) · [Gateway CI artifact identity](server-image.json) · [CI image smoke tests](https://github.com/decentraland/dcl-one/actions/runs/45379062331)

The checked catalogue contains 17 distinct sources. On the public gateway, all 15 WebP sources still failed the native-format check and both PNG controls passed. The candidate passed all 145 checks across the native and browser paths. The server then restarted with an empty process cache. Its recorded player GETs returned HTTP 200, and each expected source matched the candidate response hash before its first corresponding screenshot. [Public gateway requests](http-public-baseline.json) · [Candidate requests](http-candidate.json) · [Restart record](cold-start.json) · [Player requests](player-requests.json) · [Request-to-screenshot matches](player-receipt-matches.json)

For the 13 sources with matching source hashes in both samples, candidate rasters totaled 2,154,201 bytes and warm production KTX2 files totaled 9,078,699 bytes. The products differ in resolution and mip levels. GPU memory and time were not measured. [Pairwise transfer results](poster-transfer.json)

Chiri's community profile has its own thumbnail. That image still returned HTTP 404 on the public gateway while its upstream copy returned a PNG. The purple placeholder is visible in the local player after the event posters loaded. [Community header](chiri-community-header.png) · [Community icon issue #2060](https://github.com/decentraland/dcl-one/issues/2060)

The Linux Rust shards passed. One macOS shard failed in the LiveKit child-process and deployment-backlog checks. Both new image-rendering tests in that shard passed. A stock player against the deployed candidate remains a release check. [Before-fix regression failure](https://github.com/decentraland/dcl-one/actions/runs/45373673244/job/138374100505) · [Candidate CI](https://github.com/decentraland/dcl-one/actions/runs/45374680390) · [Issue #2062](https://github.com/decentraland/dcl-one/issues/2062)
