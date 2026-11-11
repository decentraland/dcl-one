> 创建你自己的 Decentraland 库，与他人分享

# 创建库

库是分享常见问题解决方案的绝佳方式。复杂的挑战可以只处理一次，将解决方案封装成一个库，每当它们出现时，你只需写一行代码即可。通过与社区共享库，我们可以让所有创作者的生产力成倍增长。

目前，这些库在 [示例页面](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 中可供所有人使用。我们也鼓励你创建并分享你自己的库。

按照此处详述的步骤，你可以避免创建一个兼容 Decentraland 场景且易于与他人共享的库时所带来的大部分复杂性。

## 创建一个库

> **📔 注意**：在构建你的库之前，请确保你使用的是 Node 20 或更高版本。

要创建你自己的库并通过 NPM 分享，请按以下步骤操作：

1. 创建一个新的空文件夹，并在其中打开一个新的 VS Studio Code 窗口。
2. 在 Visual Studio 左侧边栏中选择 Decentraland 选项卡
3. 点击 **创建项目**，然后选择 **库**

   这将创建 Decentraland 库所需的所有默认文件和依赖项。
4. 设置一个唯一的 `名称` 在 `package.json`。这是发布到 NPM 时将使用的名称，请确保 npm 上没有其他现有项目使用该名称。还要填写描述和任何标签，以帮助他人找到你的库。
5. 为你的项目创建一个新的 *公开* GitHub 仓库。

   该项目已配置为使用 GitHub Actions，在每次推送到时都会发布该包的新版本 `main`.

   在 `package.json` 文件，请确保在 `仓库` 中你将 `url` 字段指向你仓库的 URL。这样，浏览 [npmjs.com](https://www.npmjs.com).
6. 获取一个 NPM 令牌：
   1. 在 <https://www.npmjs.com/> 创建一个账号或登录。
   2. 前往 **账户 > 访问令牌 > 生成新令牌** 创建一个新令牌，并赋予它 **发布** 权限。

      成功消息中包含一个新生成令牌的字符串。复制该字符串并将其保存在安全的地方。
7. 在你的 GitHub 仓库中，前往 **Settings > Secrets > Actions**，然后点击 **New Repository Secret**.

   将你的密钥命名为 **NPM\_TOKEN**，并将 NPM 令牌中的字符串粘贴为其值。
8. 向 GitHub 仓库的 main 分支推送一个更改（任何更改都可以），该包就会被发布。

   就这样，现在任何使用 `npm i <package-name>`的人都可以安装该包！现在你可以在 [npmjs.com](https://www.npmjs.com).
9. 充实 `/src` 文件夹中的项目，添加你想要暴露的所有功能，并将更改推送到 GitHub。

## 开发

将你想要暴露到库中的任何函数、组件等添加到库的 `index.ts` 文件中，这样它们就很容易导入。

例如，如果你添加了一个 `RotatorComponent` 组件附加到 `index.ts`，那么你就可以通过编写以下代码将这个组件导入到一个场景中：

`import { RotatorComponent } from 'my-library'`

在开发过程中，通过在一个新的 Decentraland 场景中使用它来测试你的库。通过运行以下命令将你的 npm 包安装到场景中 `npm i <package-name>`。然后构建一个用于尝试库功能的场景。尝试用你的场景覆盖不同的测试用例，以确保你的库在每种情况下都能按预期工作。

### 快速迭代

如果你需要不断对库进行小幅调整并测试它们，那么每次都要提交更改，然后等待 npm 发布完成后才能在场景中试用新版本，这会让人筋疲力尽。幸运的是，针对你的库进行测试还有一个快得多的替代方案。

在一个 **测试场景项目**:

1. 中，将此脚本添加到 `scripts` 列表中，位于 package.json： `"link-sdk": "cd node_modules/@dcl/sdk && npm link && cd ../js-runtime && npm link"`
2. 运行 `npm install`
3. 运行 `npm install YOUR_LIBRARY_PATH`，例如， `npm install /User/projects/dcl-sdk7-library-test`
4. 运行 `npm run link-sdk`

在 **库项目**:

1. 运行 `npm install`
2. 运行 `npm run build`
3. 运行： `npm link @dcl/sdk @dcl/js-runtime`

> **❗警告**：这些步骤的顺序很重要。按其他顺序可能无法正常工作。

这会让你的场景与本地驱动器中库的版本保持同步。对于你想测试的任何库更改，只需在库文件夹上运行 `npm run build` ，无需将更改发布到 GitHub 或 NPM。

> **💡 提示**：要验证链接是否成功，请运行 `npm ls --link`。你应该会看到库名称指向本地文件中的文件夹。

如果你对库做了更改，必须运行 `npm run build` 来更新它们。为了避免每次都这样做：

1. 将脚本 "start": "tsc -p tsconfig.json --watch" 添加到库的
2. 运行 `npm start` 中

当你完成测试后，记得解除库的链接。

1. 在场景文件夹中运行 `npm install <library name>`
2. 然后在库中运行 `rm -rf node_modules && npm install`

## 版本管理

你的库的版本会自动发布到 `npm` 并使用一个 `@latest` 以及一个 `@next` 标记。

该 `@next` 标记始终指向 `main` 分支上的最后一次提交。该版本可能不稳定，因为最后的更改可能尚未经过测试。你的库的用户可以通过执行以下命令安装它（风险自负） `npm i <library name>@next`.

该 `@latest` 标记指向该库最后一个稳定版本。这才是你的库的用户通常应该安装的版本。它是 npm 在用户执行以下命令时获取的版本 `npm i <library name>`.

要让 `@latest` 标记指向你最新的提交，你需要在 GitHub 上进行一次 **发布** 你的库。

1. 打开你项目的 GitHub 页面。打开 **Releases** 链接，它位于页面右侧边栏。
2. 点击 **Draft new release**.
3. 开启 **选择标签** 为你的新版本写一个名称，例如“1.1.0”。也在 **Release Title**中写一个名称。这个名称通常相同，“1.1.0”。

   > 重要：标签必须是数字，不能带有额外的字母或符号。该数字必须高于之前已发布版本。
4. 描述你的发布内容，让用户知道有哪些新内容。

   > 提示：点击按钮 **Auto generate release notes** 以打印出自上次发布以来的所有提交。
5. 点击 **Publish Release**。此操作会触发一次新的自动发布到 npm，并带有 `@latest` 标记。你的库的用户现在将下载此版本。

## 可用性说明

尽最大努力让你的库对其他创作者来说易于使用。我们的假设总会帮助塑造我们制作的工具。很多时候，这些假设在我们看来显而易见，但在其他背景下的人可能会有完全不同的假设。

* 为库中的所有内容仔细选择名称，包括函数、组件和参数。与其用一个让人完全摸不着头脑的短名称，不如用一个自解释的长名称。
* 要广泛考虑你的库的不同可能使用场景。也许有人会在智能穿戴设备中使用你的库，也许有人需要在某些时候开启或关闭你的系统，也许有人需要在同一个场景中添加你的系统的多个实例。并不是每个人面对的挑战都和你相同。
* 话虽如此，你不需要支持所有可能的用例。事实上，灵活性与易用性之间总是需要权衡，打造好工具的关键就在于找到合适的平衡。对于你选择不支持的内容，要有策略地划定界限。不过，清楚地记录你决定采用的限制也很重要。
* 这就引出了文档。请清晰地记录你的库，提供示例、每个参数作用的说明、哪些场景可能不适用的备注，以及你对其使用上下文所做的假设。库的 README.md 往往是展示这些内容的最佳位置。
* 另一个很好的工具是直接在代码中添加元数据注释。当库的用户输入内容时，这些注释会由 IDE（如 VS Studio Code）显示出来。你可以描述每个函数/组件的用途、每个参数期望接收什么，以及函数返回什么。得益于此，库的用户无需在代码和文档之间来回切换，所有内容都在一个地方！例如，看看来自 `@dcl/ecs-scene-utils` 库中的这个函数：

  ```ts
  /**
   * 将一个值限制在最小值或最大值范围内，使其不会超出。
   *
   * @param value - 输入数值
   * @param min - 最小输出值。
   * @param max - 最大输出值。
   * @returns 最终映射后的值，位于 min 和 max 之间
   * @public
   */
  export function clamp(value: number, min: number, max: number) {
  	let result = value

  	if (value > max) {
  		result = max
  	} else if (value < min) {
  		result = min
  	}
  	return result
  }
  ```

  当库的用户输入 `clamp()` 函数：
* 时，这些提示会显示出来。
* 始终声明类型。不要让人猜测他们将需要使用什么对象结构。
* 保持你的库轻量，并专注于一项功能。当 Decentraland 场景被编译时，其所有库中的所有代码都会与它一起打包，即使是场景从未调用过的代码也是如此。因此，最好避免创建庞大笨重的库。更好的做法是创建精简的库，让创作者只使用他们真正需要的部分。
* 关注你的库仓库，别人可能会提交拉取请求或报告问题。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ku/create-libraries.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
