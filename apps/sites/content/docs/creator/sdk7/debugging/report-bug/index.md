> How you can report a bug so that it gets reviewed and fixed

# Report a Bug

If you encounter any bugs or issues with the Decentraland SDK and the Scene Editor in Creator Hub, please report them on our GitHub repository [here](https://github.com/decentraland/sdk/issues/new/choose).

## Before you report a bug

Before submitting a bug report, please ensure the following:

* Ensure you're using the latest version of the SDK, in case the issue has already been fixed in newer releases.
* Check the documentation for the feature you're trying to use, to confirm that it's supposed to behave as you're expecting, and that this is not just a misunderstanding.
* Rule out that the problem could be caused by how your scene is built. See [Debug in Preview](/docs/creator/sdk7/debugging/debug-in-preview/) and [Debug in Prod](/docs/creator/sdk7/debugging/debug-in-prod/) for tips.
* Avoid duplicates. Check the [list of known bugs](https://github.com/orgs/decentraland/projects/20/views/13) to avoid making a duplicate. If you have more information to add to an existing issue, please add a comment to the issue rather than creating a new issue.

## Reporting a bug

To report a bug with the SDK, you'll need a GitHub account. Follow these steps:

1. Visit the following link: [New Issue Page](https://github.com/decentraland/sdk/issues/new/choose)
2. Click **Get Started** next to **Bug Report**. ![](/docs/creator/images/report-bug.webp)
3. Complete all the fields in the template. The template provides instructions for each section. The more details you can provide, the quicker our developers can identify and resolve the issue.
4. Click **Submit new issue**.

![](/docs/creator/images/submit-issue.webp)

Developers may ask follow-up questions on your ticket if they need clarifications or additional information. You'll receive email notifications of updates to your bug report.


> [!WARNING]
> **📔 Note**: For any issues not related to the SDK please contact the support team. To reach out, please visit this page [intercom.decentraland.org](https://intercom.decentraland.org/) or send us an email to [hello@decentraland.org](mailto:hello@decentraland.org). You can also find us on [Discord](https://decentraland.org/discord/). The team is available to help out from Monday-Sunday, 12pm to 9pm UTC and will do its best to get back in touch as soon as possible. You can expect a reply within 60 min on Discord during the above time frame and within 24h on email.
> 
> You can also contact the DAO support team on the [DAO Discord](https://discord.gg/bxHtcMxUs4).


**Additional Tips for Bug Reports:**

* Create a separate issue for each bug. If you encounter multiple issues, make sure to create a new issue for each one.
* Avoid lengthy explanations. Only include relevant command-line or console output or scene code when necessary.
* Attach images or videos if possible. You can easily drag and drop them into the issue text area, and GitHub will handle the upload and linking.

## Using the Decentraland Playground

Providing code snippets that demonstrate the problem is extremely useful. Reproducibility is crucial. If a bug can't be reproduced, it's challenging to diagnose and fix.

You can paste code snippets within your issue text, but the best way to provide code snippets is by using the [Decentraland Playground](https://playground.decentraland.org/). Follow these steps:

1. Write your code snippet on the left, and view your scene on the right panel.
2. Click the **Share** button and paste the link into your issue report.

![](/docs/creator/images/playground/playground.webp)

Using the playground simplifies issue reproduction and ensures that anyone opening the link can see the same output, regardless of their operating system, SDK version, Node version, etc. It also facilitates quick iteration to identify affected conditions and pinpoint the issue's cause.

Keep your example as concise as possible, while still reproducing the issue. A simpler example helps eliminate ambiguity about the problem's source.


> [!WARNING]
> **📔 Note**: It's not possible to import assets to your scene on the Playground. So if reproducing the issue requires importing a 3D model or a sound file, it won't be possible to display there.


## Getting logs

It's often useful to get the logs of the Decentraland explorer when reporting a bug.

To get your logs, simply open the chat and write `/logs`. This will open a new window with the logs of the Decentraland explorer. Attach the `Player.log` file to your issue report.

## Editing Documentation

If you discover issues in the content of the Documentation website, you can submit change requests. Simply scroll down to the bottom of the page and click the **Edit this page** button. GitHub will guide you through creating a pull request with your changes. The foundation team will review your changes and publish them if they're deemed useful.

## Feature Requests and Suggestions

If you have suggestions for adding new functionality or features rather than reporting a problem, please join the [Decentraland Discord Server](https://dcl.gg/discord) and post a message in the [Creator Hub Channel](https://discord.com/channels/417796904760639509/1288888172318560326) instead of creating a GitHub Issue.
