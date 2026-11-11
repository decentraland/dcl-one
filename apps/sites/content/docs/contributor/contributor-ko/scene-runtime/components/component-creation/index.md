# 컴포넌트 생성

새 컴포넌트를 만들려면, 렌더러에서 코딩을 시작하기 전에 몇 가지 작업을 해야 합니다. 따라야 할 단계별 가이드가 있습니다. 모든 단계를 먼저 나열한 다음, 각 단계를 더 자세한 설명과 함께 따라가면 됩니다.

1. 에서 proto 정의를 만드세요 [@dcl/protocol](https://github.com/decentraland/protocol)
2. 에서 새로운 proto TypeScript 코드를 생성하세요 [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)
3. 에 대한 테스트를 만드세요 `js-sdk-toolchain`
4. 프로젝트에서 새로운 proto C# 코드를 생성하세요
5. 새 컴포넌트 코딩하기
6. 컴포넌트가 규칙을 따르도록 하세요

## 에서 proto 정의를 만드세요 [@dcl/protocol](https://github.com/decentraland/protocol)

정의를 만들려면 이 저장소로 가야 합니다: <https://github.com/decentraland/protocol>

1. 이 폴더에서 proto 정의를 만드세요: <https://github.com/decentraland/protocol/tree/main/ecs/components>
2. 새 변경 사항으로 PR을 만드세요

> ***참고:*** PR을 만든 후, GitHub Bot이 PR을 테스트할 수 있는 패키지 링크와 함께 댓글을 남길 것입니다. 이후 단계의 테스트에 그 링크를 사용할 수 있습니다

고려해야 할 사항

* 우리는 proto 3를 사용하므로, proto의 모든 정의는 다음과 함께 컴파일되어야 합니다 [구문](https://developers.google.com/protocol-buffers/docs/proto3/)
* 다시 만들 수 없는 공통 타입이 몇 가지 있습니다
* proto는 기본 정의를 가져야 합니다
* 컴포넌트를 열거하기 위해 다음 코드를 추가해야 합니다

```
import "common/id.proto";
option (ecs_component_id) = 1100;
```

proto 예시

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

## 에서 새로운 proto TypeScript 코드를 생성하세요 [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)

다음 저장소를 다운로드하세요: <https://github.com/decentraland/js-sdk-toolchain>

그 안에서 다음으로 이동하세요 `packages/dcl/ecs` (`https://github.com/decentraland/js-sdk-toolchain/tree/main/packages/%40dcl/ecs`)

그리고 그곳에서 다음을 실행할 수 있습니다

```
npm install @dcl/protocol@next
```

또는 @dcl/protocol PR에서 GitHub Bot이 생성한 명령어를 사용하세요(이는 PR 테스트를 위한 임시 명령어여야 합니다).

그다음 다음 명령어를 다음의 루트에서 실행하세요 `js-sdk-toolchain` 프로젝트:

```
make install
make build
```

그리고 생성된 코드를 push하세요.

## 프로젝트에서 새로운 proto C# 코드를 생성하세요

C# 코드를 생성하려면 다음으로 가야 합니다 `protocol-gen` 경로로 이동한 뒤, 다음 저장소의 루트에서 `@decentraland/unity-renderer` 다음 명령어를 실행하세요:

```
npm install
npm run build-components
```

다음의 최신 버전으로 업그레이드하려면 `@dcl/protocol` (main 브랜치), 다음을 사용해 업데이트해야 합니다

```
npm install @dcl/protocol@next
npm run build-components
```

PR를 테스트하려면, 다음에서 GitHub Bot이 생성한 URL을 사용할 수 있습니다 `@dcl/protocol` PR: 예시:

```
npm install "https://sdk-team-cdn.decentraland.org/@dcl/protocol/branch//dcl-protocol-1.0.0-3143233696.commit-45f1290.tgz"
npm run build-components
```

@dcl/protocol PR을 병합한 후에는 다음을 사용해야 합니다 `@dcl/protocol@next` 를 사용해 코드를 생성해야 합니다.

## 새 컴포넌트 코딩하기

이제 새 컴포넌트의 기능을 구현할 차례입니다.

컴포넌트에는 다섯 가지 핵심 부분이 있습니다.

* ComponentID 컴포넌트가 사용할 ID입니다. 고유해야 하며 proto 정의에서 생성되어야 합니다
* Model 이것은 컴포넌트의 모델입니다. 컴포넌트를 처리하는 데 사용할 데이터입니다. proto 생성 과정에서 자동으로 생성되며, proto 파일 이름 앞에 PB가 붙은 동일한 이름을 가집니다. 예를 들어 `BoxShape.proto` 정의가 있다면, 모델의 생성된 클래스는 `PBBoxShape`
* Component Handler 컴포넌트 핸들러는 컴포넌트의 모든 기능을 관리합니다. 이 클래스에서는 다음을 구현해야 합니다 `IECSComponentHandler<ModelClass>` (ModelClass는 모델입니다. 이는 proto에서 생성된 클래스이며, 이름은 PB + 파일 이름 .proto가 됩니다). 이 인터페이스에는 컴포넌트를 만들기 위해 구현해야 하는 중요한 3개의 메서드가 있습니다

```
        void OnComponentCreated(IParcelScene scene, IDCLEntity entity);
        void OnComponentRemoved(IParcelScene scene, IDCLEntity entity);
        void OnComponentModelUpdated(IParcelScene scene, IDCLEntity entity, ModelType model);
```

* Serializer 각 컴포넌트는 자신의 직렬화와 역직렬화를 구현해야 합니다. 이 serializer는 byte array로 serialize/deserialize할 수 있어야 합니다
* Register 이는 컴포넌트를 시스템에 등록하여 시스템에 연결합니다. 이 register는 factory와 component writer에 컴포넌트를 등록합니다

컴포넌트의 설계는 상속을 피하는 것이므로 가능한 한 순수 함수를 사용하는 것을 권장합니다

이를 만들려면 다음 단계를 따라야 합니다

1. 컴포넌트 폴더와 어셈블리를 만드세요. 모든 컴포넌트는 다음 폴더 아래에 있습니다 `DCLPlugins/ECS7/ECSComponents`. 컴포넌트를 보관할 폴더와 새 어셈블리를 만들어야 합니다
2. 새 어셈블리에서는 다음을 참조해야 합니다 `DCL.ECSComponents.Data`. 이것은 방금 업데이트한 컴포넌트의 새 모델을 참조합니다
3. 모든 로직이 포함된 컴포넌트 핸들러를 만들어야 합니다(예를 들어 다음을 살펴보세요 `ECSBoxShapeComponentHandler.cs` 를 예시로)
4. serializer 클래스를 만들어야 합니다(아마 다른 클래스에서 복사한 뒤 자신에게 맞게 수정할 수 있습니다)
5. register 클래스를 만들어야 합니다

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

6. 새 register를 다음에 추가하세요 `ECS7ComponentsComposer` 클래스에 해당 ID와 함께

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

이제 컴포넌트가 추가되어 동작합니다!

## 컴포넌트가 규칙을 따르도록 하세요

새 컴포넌트를 개발할 때 고려해야 할 체크리스트가 있습니다. 이 부분은 그것들을 요약하려고 합니다.

* 단위 테스트 모든 컴포넌트에는 기능, dispose, 그리고 최소한 직렬화/역직렬화를 검증하는 단위 테스트가 포함되어야 하며, 이를 통해 컴포넌트가 동작함을 보장해야 합니다
* 컴포넌트가 씬 안에 없을 때 어떤 일이 일어나는지 고려하세요( `SceneBoundariesChecker` 클래스를 참고하세요)
* 컴포넌트가 월드에 무언가를 렌더링한다면, 렌더 가능한 정보를 데이터 저장소에 추가해야 합니다. 이렇게 하면 렌더러의 정보를 씬에 추가하여 한도 계산에 반영할 수 있습니다
* 컴포넌트가 월드에 무언가를 렌더링한다면, 다음을 추가해야 합니다 `MeshesInfo` 를 엔티티에
* 가능한 한 성능이 좋아야 합니다. 이 코드는 매우 자주 실행되므로 모든 것이 최대한 부드럽게 동작하도록 보장해야 합니다
* 다음과 함께 동작해야 합니다 `핫 리로드` 미리보기 모드에서. 만약 `OnComponentRemoved` 를 올바르게 코딩했다면, 이것은 별도의 작업 없이 동작하겠지만, 핫 리로드는 컴포넌트의 dispose와 함께 모든 것이 잘 동작하는지 테스트하는 방법입니다
* 컴포넌트가 리소스를 사용한다면, 다음을 사용하여 리소스 관리를 구현해야 합니다 `AssetPromiseKeeper`. 컴포넌트는 리소스가 사용될 때와 더 이상 사용되지 않을 때 이를 `AssetPromiseKeeper` 알려야 합니다


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/components/component-creation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
