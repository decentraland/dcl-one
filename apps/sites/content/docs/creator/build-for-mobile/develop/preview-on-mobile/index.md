> Preview your scene on a real mobile device from the Creator Hub or the CLI.

# Preview Your Scene on Mobile

You can preview your scene directly on the Decentraland mobile app from the Creator Hub or from the command line. This is the only reliable way to confirm that your UI, input handling, and performance hold up on a real device.

## Prerequisites

* The Decentraland mobile app installed on your phone — see the [download links](/docs/creator/build-for-mobile/mobile-client/overview/#get-the-mobile-app).
* The phone and your development machine must be on the **same local network** (Wi-Fi). The preview is served from your computer; the QR code links to a LAN URL.

## Option A — From the Creator Hub

1. Open your scene in the Creator Hub.
2. Click the dropdown next to the **Preview** button and choose **Show QR Code for Mobile**.
3. Scan the displayed QR code with your phone's camera. The link opens the Decentraland mobile app and loads your scene preview.

<figure><img src="/docs/.gitbook/assets/mobile-preview-creator-hub.webp" alt="Creator Hub preview dropdown with the Show QR Code for Mobile option"><figcaption><p>The "Show QR Code for Mobile" option in the Creator Hub preview dropdown.</p></figcaption></figure>

## Option B — From the command line

From the root of your scene project, run:

```bash
npm run start -- --mobile
```

The CLI prints a QR code in the terminal that points to your scene's LAN URL. Scan it with your phone to load the scene in the Decentraland mobile app.

<figure><img src="/docs/.gitbook/assets/mobile-preview-cli-qr.webp" alt="QR code printed by the Decentraland CLI for mobile preview"><figcaption><p>A QR code printed by <code>npm run start -- --mobile</code>. Scan it with your phone to open the scene in the Decentraland mobile app.</p></figcaption></figure>


> [!NOTE]
> **💡 Tip**: When you pass `--mobile`, the desktop explorer is not also launched. If you want to test on both at once, run `npm run start` in one terminal for desktop and `npm run start -- --mobile` in another for mobile.


## Hot reload

Just like with desktop preview, the mobile preview reloads automatically when you change scene files. You don't need to re-scan the QR code after every edit.

## Troubleshooting

* **QR code doesn't open the app** — make sure the Decentraland app is installed and that you've opened it at least once on the device.
* **Phone can't reach the preview server** — confirm the phone and the dev machine are on the same Wi-Fi network. Some corporate networks isolate clients; use a personal hotspot or a different network if so.
* **Scene loads on the phone but looks different from desktop** — that is expected. Use this as the moment to validate your [safe area](/docs/creator/build-for-mobile/develop/safe-area/), [UI sizing](/docs/creator/build-for-mobile/develop/ui-best-practices/#sizing), and [input bindings](/docs/creator/build-for-mobile/develop/input-on-mobile/).

## Related

* [Preview Your Scene](/docs/creator/sdk7/getting-started/preview-scene/)
* [Using the CLI](/docs/creator/sdk7/getting-started/using-the-cli/)
* [Get featured on mobile Discover](/docs/creator/build-for-mobile/publish/get-featured/)
