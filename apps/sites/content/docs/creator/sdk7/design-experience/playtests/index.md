> How to run playtests of your Decentraland game and turn player feedback into improvements

# Playtest Your Games

Playtesting helps you make better games by finding out how real players experience them.

You don't need a finished game or a large group of testers to get started. You can test early, make improvements, and test again as your game develops.

This guide explains how to organize playtests, collect player feedback, and use what you learn to improve your game.

## Why playtest your game?

When you build a game, you already know how it works. New players don't.

Playtesting helps you discover things you might otherwise miss:

- **Clarity:** Do players understand what to do?
- **Ease of use:** Can players complete actions without getting stuck?
- **Enjoyment:** Which parts are fun, and which are frustrating?
- **Motivation:** Do players want to keep playing or come back?

**Start playtesting as early as possible.** You can learn a lot from watching just a few people try your first playable version.

## Choose what you want to test

Before each playtest, decide what you want to learn.

Focus on one or two questions rather than trying to test everything at once.

| Your goal              | What to look for                                             |
| ---------------------- | ------------------------------------------------------------ |
| Test onboarding        | Can new players understand what to do without help?          |
| Test the core mechanic | Can players use the main game mechanic? Is it enjoyable?     |
| Test the game loop     | Do players understand how to make progress and earn rewards? |
| Test difficulty        | Where do players get stuck, bored, or frustrated?            |
| Test emotional response | Do players feel excited, curious, or eager to keep playing? Which moments create the strongest reactions? |

Choose players who are unfamiliar with your game whenever possible. They are more likely to find problems that experienced players no longer notice.

### Match the playtest to your game's stage

Decide what kind of response you're looking for. Early on, you mostly need to know whether players **understand** the game and can form a **strategy**: do they know what to do, and can they work out how to do it well? Once that's solid, you need to know how the game makes them **feel**: is it exciting, satisfying, or tense in the right moments, and do they want to come back?

The kind of player you invite changes with the stage too. Someone who's comfortable with Decentraland will get past the client and straight into your game, so they're ideal when you're testing mechanics. A complete newcomer stumbles on everything, which is exactly what you want once you're testing the full first-time experience.

| Stage                | What you want to learn                                                                 | Who to invite                                                                                  |
| -------------------- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| V0 (vertical slice)  | Is the core mechanic understandable and usable? Can players form a strategy around it? | A few players who already know Decentraland, so the client doesn't get in the way of the test. |
| V1 (first full loop) | Can players progress on their own? Where do they get stuck or lose interest?           | A mix of Decentraland players and people who have never played your game.                      |
| V2 and beyond        | How does the game make players feel? Do they come back? Would they recommend it?       | New players, including people from outside the Decentraland community.                         |

Write your questions for the stage you're at. Asking "did you have fun?" about a V0 slice tells you little, because there's not enough game yet for the answer to mean much. Asking "did you understand what to do?" about a V2 that's already been through several rounds wastes a session on something you already know.

## Collect feedback automatically with Scene Feedback

Scene Feedback is a reusable module that lets you ask players questions directly inside your Decentraland game.

It helps you understand how players experience your game, even when you're not there to watch them play.

You can:

- Ask players questions at specific moments during gameplay.
- Collect 1 to 5 ratings and written comments.
- Download responses as a CSV file from your World's Scene Storage.

### What you need first

Scene Feedback stores answers on a server, so your scene needs a [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/), enabled by using `@dcl/sdk@auth-server` and setting `"authoritativeMultiplayer": true` in `scene.json`.

### Install Scene Feedback

1. Open the [Scene Feedback repository](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback).
2. Follow the README to install the module in your scene. You can also give the installation prompt to your AI coding agent.
3. Choose a small set of questions that match your playtest goals.
4. Decide when each question should appear.
5. Download player responses as a CSV file. In Creator Hub, open **Manage**, then your World's **⋮** menu, then **View Storage**. Responses may be spread across several files for a while (newer saves that aren't merged yet, plus an older single file from earlier versions of the module). Download every file whose name starts with the same `scene-feedback-<number>`, and drop rows with duplicate `id`s.

The repository includes ready-to-use questions, examples, and guidance for choosing questions.

If you use an AI coding assistant, install or update the [Decentraland SDK Skills](/docs/creator/sdk7/getting-started/vibe-coding/). They include a Scene Feedback skill that can help you decide what to ask and when, install the module, and analyze the responses afterwards:

```bash
npx skills add decentraland/sdk-skills --all
```

## Organize your playtest

You can organize a playtest in two ways.

**Option 1: Host a live playtest**

Invite everyone to play at the same time. You can observe players, discuss their experiences, and collect feedback afterwards.

You can also create an event on [Decentraland Events](https://decentraland.org/events/) so players can find and join your playtest.

**Option 2: Run a playtest over several days**

Let players test your game whenever they're available during a set period.

Install Scene Feedback to collect responses while they play.

This is useful when you want feedback from more players without organizing a live session.

## Host a live playtest

A live playtest lets you watch players experience your game and ask them questions afterwards.

### Step 1. Prepare your game

Make sure players can access your game and complete the actions you want to test. Prepare two or three tasks for players to try.

For example:

- Find out how to start the game.
- Complete one round.
- Buy an upgrade.

### Step 2. Invite playtesters

Invite a small group of players. Three to five players is a useful starting point. Try to include people who haven't played your game before.

You can find interested players in the #playtesters channel on the [Decentraland Discord](https://discord.gg/decentraland). See [Find playtesters](#find-playtesters) for other places to look.

### Step 3. Watch players play

Ask players to try the game without explaining how everything works. Watch what they do, where they get stuck, and what they enjoy.

**Don't guide players unless they cannot continue.** If you have to explain something, that may be a sign your game needs clearer instructions.

### Step 4. Ask for feedback

After playing, ask a few open questions:

- What did you enjoy most?
- Was anything confusing or frustrating?
- Was there a moment when you didn't know what to do?
- Would you play again? Why or why not?
- Would you recommend this game to a friend? Why or why not?

Avoid leading questions such as "Did you like the new upgrade system?" These can influence the answers you receive.

If you're not sure what to ask, the Scene Feedback skill mentioned above can walk you through choosing a focus and wording the questions, whether you plan to ask them in person or in-game.

Write down the problems players experienced, not just their suggestions.

## Improve your game with the RITE method

RITE stands for **Rapid Iterative Testing and Evaluation**.

It's a simple approach to playtesting: find a problem, fix it, and test again as quickly as possible. Instead of running one big playtest and collecting a long list of issues, you run several small ones and fix the clearest problems between each.

### Follow the RITE cycle

1. **Test:** Watch a small group of players try your game. Three to five players per round is enough.
2. **Identify:** Find problems that affect their experience.
3. **Fix:** Between rounds, make a change when the problem and solution are both clear.
4. **Repeat:** Run the next round with new players. Continue until players can complete the experience without major problems.

For example, if players cannot find where to start your game, make the Start button clearer before the next playtest. Then watch whether new players can find it without help.

**Keep the game stable while a round is running.** RITE means fixing between rounds, not during them. If you redeploy while players are mid-session, you can break their game, and you can no longer tell which version a piece of feedback refers to. Collect everything from one round, then make your changes, then test again.

### Make RITE work in Decentraland

A few habits make RITE practical for a Decentraland scene:

- **Fix small, fix fast.** Between rounds, only change what the last round clearly showed to be a problem. If a fix needs a new mechanic or a redesign, note it for later and keep the round-to-round changes small enough to ship in a day.
- **Use a World for testing.** Publish your test builds to a [World](/docs/creator/sdk7/publishing/publishing-options/) rather than to your live LAND or your main World. Players always land on a known build, and you can republish between rounds without touching a published game.
- **Keep your Scene Feedback questions the same across rounds.** If you change the wording of a question, you can't compare the ratings from one round to the next. Add questions if you need to, but don't rewrite the ones you're tracking.
- **Decide what counts as "clear".** A problem is clear when you saw it happen, you can say what the player expected instead, and you know what to change. If only one player hit it and you're not sure why, leave it for the next round and see whether it happens again.
- **Log every round.** Keep a short note per round: which build, who played, what you saw, what you changed. By the third round this log tells you whether a change actually worked.

RITE works especially well during early development, when you can make changes quickly.

## Find playtesters

Regular Decentraland players are the easiest to reach, but also look [outside the Decentraland community](#outside-the-decentraland-community): they catch problems that experienced players miss.

### On Discord

The #playtesters channel on the [Decentraland Discord](https://discord.gg/decentraland) is for people who enjoy testing games and helping creators improve them. Join the server and find the channel.

### Post your playtest

When you're ready, post a message on #playtesters that includes:

- **Game:** The name of your game and a short description.
- **Location:** A link to your game in Decentraland.
- **When:** The date and time of your playtest, or the period when feedback is open.
- **What to test:** The specific features or gameplay you want feedback on.
- **How to give feedback:** Through live discussion, Discord, or Scene Feedback.
- **Rewards:** Any incentive you're offering for participation.

### Encourage players to participate

Consider offering a small reward to thank players for their time.

For example:

- Gift participants a special Wearable or Emote.
- Offer an exclusive in-game reward. The Scene Feedback example includes a recipe that shows a **Playtest Contributor** label over the player's avatar with [`AvatarNametag`](/docs/creator/sdk7/interactivity/avatars/avatar-nametags/) once they answer every question in a form. The label is only visible to that player and lasts for their visit. See [A scene-local reward for participation](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0%2C-1-scene-feedback/EXAMPLES.md#6-a-scene-local-reward-for-participation).
- Give eligible players Marketplace Credits.

**Gifting Wearables and Emotes.** You can give any Wearable or Emote you own to another player directly from the desktop client: right-click the player and choose **Gift**. The item is transferred to their account and appears in their Backpack. You can also transfer an item you own from the [Marketplace](https://decentraland.org/marketplace/) using the recipient's wallet address. If you created the Wearable or Emote collection, you can mint items straight to any wallet address from the Builder, even if you don't run into the player in-world. See [Minting Wearables](/docs/creator/wearables-and-emotes/publishing/publishing-collections/#minting-wearables).

**Marketplace Credits gifting is currently in beta and available to invited studios.** Eligible studios can gift Credits to players, who can spend them in the Decentraland Shop. See [Gifting Credits (Beta)](/docs/creator/rewards/gifting-credits/).

Rewards can encourage participation, but make it clear that you want genuine feedback, including criticism.

### Outside the Decentraland community

You can also invite people who've never used Decentraland. Decentraland players already know how the client works, so they'll go straight to your game. People from outside see everything with fresh eyes, including the parts you've stopped noticing. Playtesting with them is also a good way to introduce new players to Decentraland.

Indie developers commonly recruit from:

- **Friends and family.** Easy to reach, and happy to give you an hour. Ask them to be honest and to tell you when they're confused.
- **People who don't usually play games.** If they can get through your onboarding, almost anyone can.
- **Communities you're already part of.** A game jam server, a hobby forum, a local meetup, or a group that matches your game's theme.

If your testers are new to Decentraland, help them get in: share the web link to your scene (`https://decentraland.org/jump/?realm=<your-world-name>.dcl.eth` for a World, or `https://decentraland.org/jump/?position=<x>,<y>` for LAND), which the Creator Hub shows after you publish, plus a short note on installing the client and signing in. The link opens in a browser even before the client is installed. Keep notes on where they struggle before reaching your scene, but treat that separately from feedback about your game.

## Use feedback to improve your next version

After each playtest, review what you learned before deciding what to build next.

1. **Group similar problems.** Look for issues that multiple players experienced.
2. **Prioritize important issues.** Fix anything that stops players from understanding, progressing, or enjoying the game.
3. **Make improvements.** Start with changes that are quick to implement and likely to help.
4. **Test again.** Check whether new players have a better experience.
5. **Track your changes.** Keep a short record of what you changed and what happened in the next playtest.
6. **Share what changed.** Let your playtesters know what you learned and what you've improved because of their feedback. Post an update in your original Discord thread so they can see the impact of their contribution.

Remember that what players **do** can be different from what they **say**. Use live observations, Scene Feedback responses, and gameplay data together where possible.

## Share your improvements with playtesters

Players are more likely to participate again when they can see that their feedback made a difference. After making changes, return to your playtest post in #playtesters and share a short update.

Use this simple format: **You said, we learned, we changed.**

### Example: Playtest results

**Playtest Update: [Game Name]**

Thanks to everyone who tested our game!

**You said:** It wasn't clear how to start the first round.

**We learned:** New players couldn't easily find the Start button.

**We changed:** We've made the button bigger and added clearer instructions.

**Play again:** [Link to your updated game]

You don't need to fix every issue before posting an update. You can also explain what you're still working on or what you'd like to test next.

## Keep playtesting

Playtesting isn't something you do once before launch.

Test when you introduce a new mechanic, change onboarding, adjust difficulty, or add new content. Even small playtests can help you discover important problems before they affect more players.

**The goal isn't to prove that your game works. It's to discover how to make it better.**

## Resources

- [Scene Feedback module and installation guide](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)
- [Decentraland Discord](https://discord.gg/decentraland)
- [Decentraland Events](https://decentraland.org/events/)
- [RITE method: Rapid Iterative Testing and Evaluation](https://handbook.gitlab.com/handbook/upstream-studios/experience-research/rite/)
