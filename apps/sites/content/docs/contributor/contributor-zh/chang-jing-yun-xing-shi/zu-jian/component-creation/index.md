# 组件创建

要创建新组件，在开始在渲染器中编写代码之前，我们需要先做几件事。你需要按照一个逐步指南来操作。我会列出所有步骤，然后你可以通过更详细的说明逐步执行。

1. 在以下位置创建 proto 定义 [@dcl/protocol](https://github.com/decentraland/protocol)
2. 在以下位置生成新的 proto TypeScript 代码 [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)
3. 为以下内容创建测试 `js-sdk-toolchain`
4. 在项目中生成新的 proto C# 代码
5. 编写新组件代码
6. 确保该组件遵循约定

## 在以下位置创建 proto 定义 [@dcl/protocol](https://github.com/decentraland/protocol)

要创建定义，你必须前往此仓库：<https://github.com/decentraland/protocol>

1. 在此文件夹中创建 proto 定义：<https://github.com/decentraland/protocol/tree/main/ecs/components>
2. 用这些新变更创建一个 PR

> ***注意：*** 在创建 PR 后，GitHub Bot 会评论一个用于测试 PR 的包链接。你可以在接下来的步骤中使用该链接进行测试

需要考虑的事项

* 我们使用的是 proto 3，因此 proto 的所有定义都必须能够按其 [语法](https://developers.google.com/protocol-buffers/docs/proto3/)
* 我们有一些无法重新创建的公共类型
* proto 应该具有基本定义
* 你必须添加以下代码来枚举该组件

```
import "common/id.proto";
option (ecs_component_id) = 1100;
```

.proto 示例

```
syntax = "proto3";

import "common/id.proto";
option (ecs_component_id) = 1020;

message PBAudioSource {
  optional bool playing = 1;
  optional float volume = 2; // default=1.0f
  optional bool loop = 3;
  optional float pitch = 4; // default=1.0f
  string audio_clip_url = 5;
}
```

## 在以下位置生成新的 proto TypeScript 代码 [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)

下载以下仓库：<https://github.com/decentraland/js-sdk-toolchain>

在其中，导航到 `packages/dcl/ecs` (`https://github.com/decentraland/js-sdk-toolchain/tree/main/packages/%40dcl/ecs`)

然后在那里，你可以执行

```
npm install @dcl/protocol@next
```

或者使用你在 @dcl/protocol PR 中由 GitHub Bot 生成的命令（这必须是临时的，用于测试 PR）。

然后在以下内容的根目录下运行下面的命令 `js-sdk-toolchain` 项目：

```
make install
make build
```

然后推送生成的代码。

## 在项目中生成新的 proto C# 代码

要生成 C# 代码，我们需要前往 `protocol-gen` 路径，位于以下仓库的根目录中 `@decentraland/unity-renderer` 仓库。并执行以下命令：

```
npm install
npm run build-components
```

要升级到最新版本的 `@dcl/protocol` （main 分支），我们应使用以下方式更新

```
npm install @dcl/protocol@next
npm run build-components
```

要测试 PR，我们可以使用 GitHub Bot 在以下位置生成的 URL `@dcl/protocol` PR：示例：

```
npm install "https://sdk-team-cdn.decentraland.org/@dcl/protocol/branch//dcl-protocol-1.0.0-3143233696.commit-45f1290.tgz"
npm run build-components
```

在合并 @dcl/protocol PR 之后，我们必须使用 `@dcl/protocol@next` 并生成代码。

## 编写新组件代码

现在该实现新组件的功能了。

组件有五个基本部分。

* ComponentID：组件将使用的 ID。它必须是唯一的，并由 proto 定义生成
* Model：这是组件的模型。这是我们用于处理组件的数据。它是通过 proto 生成自动生成的，并且与 proto 文件同名，只是在前面加上 PB。例如，如果你有一个 `BoxShape.proto` 定义，那么该模型生成的类将是 `PBBoxShape`
* Component Handler：组件处理器将管理组件的所有功能。在这个类中，你必须实现 `IECSComponentHandler<ModelClass>` （ModelClass 是模型。它是从 proto 生成的类，名称将是 PB + 文件名 .proto）。为了创建组件，这个接口有 3 个重要方法需要实现

```
        void OnComponentCreated(IParcelScene scene, IDCLEntity entity);
        void OnComponentRemoved(IParcelScene scene, IDCLEntity entity);
        void OnComponentModelUpdated(IParcelScene scene, IDCLEntity entity, ModelType model);
```

* Serializer：每个组件都负责实现其序列化和反序列化。这个序列化器必须能够将其序列化/反序列化为字节数组
* Register：这会将组件注册到系统中，并将其与系统连接起来。这个注册器会在工厂和组件写入器中注册该组件

组件的设计是为了避免继承，因此我们鼓励尽可能使用纯函数

为了创建它们，我们必须遵循以下步骤

1. 创建组件文件夹和程序集。我们所有的组件都位于以下文件夹下 `DCLPlugins/ECS7/ECSComponents`。你需要创建一个文件夹和一个新的程序集来承载该组件
2. 在新的程序集里，你必须引用以下程序集 `DCL.ECSComponents.Data`。这将引用你刚刚更新的组件新模型
3. 你必须创建包含全部逻辑的组件处理器（请查看 `ECSBoxShapeComponentHandler.cs` 作为示例）
4. 你必须创建序列化器类（你可能可以从另一个类复制并根据你的需要进行调整）
5. 你必须创建注册器类

```sh
   public static class AudioSourceSerializer
    {
        public static byte[] Serialize(PBAudioSource model)
        {
            int size = model.CalculateSize();
            byte[] buffer = new byte[size];
            CodedOutputStream output = new CodedOutputStream(buffer);
            model.WriteTo(output);
            return buffer;
        }

        public static PBAudioSource Deserialize(object data)
        {
            return PBAudioSource.Parser.ParseFrom((byte[])data);
        }
    }
```

6. 将新的注册器添加到 `ECS7ComponentsComposer` 类中，并使用其对应的 ID

```sh
   public class ECS7ComponentsComposer : IDisposable
    {
        private readonly TransformRegister transformRegister;
        private readonly SphereShapeRegister sphereShapeRegister;
        private readonly BoxShapeRegister boxShapeRegister;
        private readonly PlaneShapeRegister planeShapeRegister;
        private readonly CylinderShapeRegister cylinderShapeRegister;
        private readonly AudioStreamRegister audioStreamRegister;
        private readonly AudioSourceRegister audioSourceRegister;

        public ECS7ComponentsComposer(ECSComponentsFactory componentsFactory, IECSComponentWriter componentsWriter)
        {
            transformRegister = new TransformRegister(ComponentID.TRANSFORM, componentsFactory, componentsWriter);
            sphereShapeRegister = new SphereShapeRegister(ComponentID.SPHERE_SHAPE, componentsFactory, componentsWriter);
            boxShapeRegister = new BoxShapeRegister(ComponentID.BOX_SHAPE, componentsFactory, componentsWriter);
            planeShapeRegister = new PlaneShapeRegister(ComponentID.PLANE_SHAPE, componentsFactory, componentsWriter);
            cylinderShapeRegister = new CylinderShapeRegister(ComponentID.CYLINDER_SHAPE, componentsFactory, componentsWriter);
            audioStreamRegister = new AudioStreamRegister(ComponentID.AUDIO_STREAM, componentsFactory, componentsWriter);
            audioSourceRegister = new AudioSourceRegister(ComponentID.AUDIO_SOURCE, componentsFactory, componentsWriter);
        }

        public void Dispose()
        {
            transformRegister.Dispose();
            sphereShapeRegister.Dispose();
            boxShapeRegister.Dispose();
            planeShapeRegister.Dispose();
            cylinderShapeRegister.Dispose();
            audioStreamRegister.Dispose();
            audioSourceRegister.Dispose();
        }
    }
```

现在你的组件已经添加并可以正常工作了！

## 确保该组件遵循约定

在开发新组件时，我们需要考虑一些检查清单，这部分尝试对其进行总结。

* 单元测试：所有组件都必须包含单元测试，至少覆盖其功能、释放以及序列化/反序列化，以确保组件能够工作
* 要考虑当组件不在场景中时会发生什么（请查看 `SceneBoundariesChecker` 类以了解更多信息）
* 如果组件向世界渲染某些内容，它必须将可渲染信息添加到数据存储中，这样我们就能将渲染器信息添加到场景中，使其计入限制
* 如果组件向世界渲染某些内容，它必须添加 `MeshesInfo` 到实体中
* 它必须尽可能高性能。此代码将被多次执行，因此我们需要确保一切尽可能流畅地运行
* 它必须能够与 `热重载` 在预览模式下配合使用。如果你已经编写了 `OnComponentRemoved` 正确，这将开箱即用，但热重载是一种测试所有内容在组件释放时是否正常工作的方式
* 如果组件使用资源，你必须使用一个来实现资源管理 `AssetPromiseKeeper`。组件应该在以下情况下进行通知 `AssetPromiseKeeper` 当资源被使用以及不再被使用时


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/zu-jian/component-creation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
