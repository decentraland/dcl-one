> 新场景中创建的默认文件。

# 场景文件

之后 [创建一个新场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/)，场景文件夹将包含一系列带有默认内容的文件。

## 场景中的默认文件

场景包含以下文件：

* **src/index.ts**：场景的入口点。
* **scene.json**：包含场景元数据的清单。
* **package.json** 和 **package-lock.json**：指定场景所有依赖项的版本。
* **tsconfig.json**：TypeScript 配置文件。
* **.dclignore**：列出你在发布场景时不会上传的项目文件。
* **assets/scene/main.composite**：自动生成的文件，包含你在 Creator Hub 的 Scene Editor 中通过可视化方式添加和配置的所有内容。它并不适合人类阅读。这是一个 [复合文件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites/)，一种描述实体和组件树状结构的文件。通过 CLI 创建的场景则包含一个 `main.crdt` 位于根目录的文件。

### index.ts

这是你场景自定义代码的入口点。你可以把整个场景逻辑都放在这个文件中，不过为了更清晰起见，在大多数情况下我们建议将代码拆分到其他几个 *.ts* 文件中。

如果你只依赖 Scene Editor 和 [Smart Items](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)，则无需修改此文件。

如果你打算编写自定义代码，通常只需编辑这个文件以及其他 .ts 文件来创建你的场景。它包含用于生成以下内容的代码： [实体、组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/) 和 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) ，以及你场景中的这些内容。

运行场景时，你的 `.ts` 文件会被编译成一个单一的 `.js` 文件， `bin/index.js`。在构建生产版本时，该文件也会被压缩。


> [!WARNING]
> **📔 注意**：你可以使用其他工具或语言代替 TypeScript，只要你的脚本包含在单个 JavaScript 文件（bin/index.js）中即可。所有提供的类型声明均使用 TypeScript 编写，其他语言和转译器不受官方支持。


### scene.json

该 *scene.json* 文件是世界中某个场景的 JSON 格式清单。一个场景可以覆盖一个或多个 LAND 地块。该 *scene.json* 清单描述场景中有哪些对象、渲染它所需资产的列表、地块所有者的联系信息以及安全设置。

该 *scene.json* 文件中的大多数字段都可以直接在 Scene Editor 的 UI 中编辑。参见 [场景设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#scene-settings).

![](/docs/_remote/0d1050cf72d4ecbad89ddda0.webp)

更多信息请参见 [场景元数据](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#metadata).

这些元数据在本地预览场景时都是可选的，但部署时其中一部分是必需的。你可以随时手动更改这些信息。

### package.json

此文件向 NPM 提供信息，使其能够识别项目并处理项目依赖。Decentraland 场景需要一个主包：

* **@dcl/sdk**：Decentraland SDK 的基础依赖项，包括引擎、组件、系统等的定义和类型。
* **@dcl/js-runtime**：一系列类型声明，使 @dcl/sdk 的集成更加顺畅。

你的场景可以包含任意数量的其他包，例如用于引入 [库](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) ，这些库可以帮助简化代码编写，或启用特殊功能。

### package-lock.json

此文件列出了项目中所有其他依赖项的版本。这些版本是锁定的，这意味着编译器将严格使用此处列出的同一小版本。

你可以通过编辑此文件手动更改任何包版本。

### tsconfig.json

包含 *tsconfig.json* 文件的目录是 TypeScript 项目的根目录。该 *tsconfig.json* 文件指定将项目从 TypeScript 编译为 JavaScript 所需的根文件和选项。

为场景安装任何额外库时，此文件中应自动添加相应条目。对于安装 Decentraland utils 库，通常无需手动修改此文件。

## 推荐的文件位置

请记住，当你将场景部署到 Decentraland 时，使用场景所需的任何资源或外部库都必须打包在场景文件夹内，或可通过远程服务器访问。

任何需要在玩家客户端中运行的内容都必须位于场景文件夹内。你不应该引用本地机器上其他位置安装的文件或库，因为它们在已部署的场景中不可用。

我们建议统一使用以下文件夹名称来存放场景可能需要的不同类型资源：

* 3D 模型： `assets/scene/models`
* 视频： `assets/scene/videos`
* 声音文件： `assets/scene/sounds`
* 用于纹理的图像文件（glTF 模型除外）： `assets/scene/materials`
* *.ts* 组件定义 `/src/components`
* *.ts* 系统定义 `/src/systems`


> [!WARNING]
> **📔 注意**：glTF 模型的辅助文件，例如其纹理图像文件或 *.bin* 文件，应始终放在与模型的 *.gltf* 或 *.glb* 文件中描述。



> [!WARNING]
> **📔 注意**：我们建议所有文件夹和文件名始终使用小写，以避免可能的问题。


通过 Scene Editor 导入任何资源时，它们会自动添加到 `assets/scene` 文件夹中。使用 Scene Editor 的资源包中的任何默认资源时，它们的文件会被添加到 `assets/builder` 文件夹中。

## dclignore 文件

所有场景都包含一个 *.dclignore* 文件；此文件指定在将场景部署到 Decentraland 时要忽略场景文件夹中的哪些文件。

例如，你可能希望将场景中 3D 模型的 Blender 文件保留在场景文件夹内，但又想阻止这些文件被部署到 Decentraland。在这种情况下，你可以添加 `*.blend` 设置为 *.dclignore* 以忽略所有具有该扩展名的文件。


> [!WARNING]
> **📔 注意**: *.dclignore* 也适用于本地预览，而不只是部署。被忽略的文件不会列在预览提供的内容中，因此你的场景在预览中也无法加载它。它并不是你能在日志中看到的 404：该文件根本不会被请求。如果某个资源在编辑器中可用但在预览中不可用，请先检查 *.dclignore* 这个。


| 要忽略的内容 | 示例          | 说明                                        |
| ------ | ----------- | ----------------------------------------- |
| 特定文件   | `BACKUP.ts` | 忽略特定文件                                    |
| 文件夹    | `drafts/`   | 忽略整个文件夹及其子文件夹中的全部内容                       |
| 扩展名    | `*.blend`   | 忽略所有具有指定扩展名的文件                            |
| 名称片段   | `test*`     | 忽略所有名称与查询匹配的文件。在这种情况下，即以以下内容开头的文件： *test* |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-files.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
