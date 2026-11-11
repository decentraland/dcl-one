> 새 씬에서 생성되는 기본 파일.

# 씬 파일

이후 [새 장면을 만들면](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/), 장면 폴더에는 기본 내용이 포함된 여러 파일이 생성됩니다.

## 장면의 기본 파일

장면에는 다음 파일이 포함됩니다:

* **src/index.ts**: 장면의 진입점입니다.
* **scene.json**: 장면의 메타데이터를 포함하는 매니페스트입니다.
* **package.json** 및 **package-lock.json**: 장면의 모든 종속성 버전을 지정합니다.
* **tsconfig.json**: TypeScript 구성 파일입니다.
* **.dclignore**: 장면을 게시할 때 업로드되지 않을 프로젝트 파일 목록입니다.
* **assets/scene/main.composite**: Creator Hub의 Scene Editor에서 시각적으로 추가하고 구성한 모든 내용을 포함하는 자동 생성 파일입니다. 사람이 읽도록 만든 것이 아닙니다. 이것은 다음의 한 예입니다: [컴포지트](/docs/creator/content-creator-ko/sdk7/architecture/composites/), 엔티티와 컴포넌트의 트리를 설명하는 파일입니다. CLI로 생성된 장면에는 대신 다음 파일이 포함됩니다: `main.crdt` 파일이 루트 수준에 있습니다.

### index.ts

이 파일은 장면의 사용자 지정 코드에 대한 진입점입니다. 전체 장면의 로직을 이 파일 하나에 모두 담을 수도 있지만, 명확성을 위해 대부분의 경우 코드를 여러 다른 *.ts* 파일에 나누어 배치하는 것을 권장합니다.

Scene Editor와 [Smart Items](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/), 이 파일을 수정할 필요가 없습니다.

사용자 지정 코드를 작성할 계획이라면, 장면을 만들기 위해 이 파일과 다른 .ts 파일만 수정하면 되는 경우가 대부분입니다. 여기에는 다음을 생성하는 코드가 들어 있습니다: [엔티티, 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/) 및 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 을(를) 생성하는 코드가 들어 있습니다.

장면을 실행할 때, 다음의 내용은: `.ts` 파일은 단일 `.js` 파일로 컴파일됩니다, `bin/index.js`. 프로덕션용으로 빌드할 때 이 파일은 또한 축소됩니다.


> [!WARNING]
> **📔 참고**: TypeScript 대신 다른 도구나 언어를 사용할 수 있지만, 스크립트가 하나의 JavaScript 파일(bin/index.js) 안에 들어 있어야 합니다. 제공되는 모든 타입 선언은 TypeScript로 작성되며, 다른 언어와 트랜스파일러는 공식적으로 지원되지 않습니다.


### scene.json

다음 *scene.json* 파일은 월드의 장면을 위한 JSON 형식의 매니페스트입니다. 장면은 하나 또는 여러 LAND 구획에 걸칠 수 있습니다. 매니페스트는 *scene.json* 장면에 어떤 오브젝트가 존재하는지, 렌더링에 필요한 에셋 목록, 구획 소유자의 연락처 정보, 보안 설정이 설명됩니다.

다음의 대부분 필드는 *scene.json* 파일을 Scene Editor의 UI에서 직접 편집할 수 있습니다. 다음을 참조하세요: [장면 설정](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#scene-settings).

![](/docs/_remote/0d1050cf72d4ecbad89ddda0.webp)

자세한 내용은 다음을 참조하세요: [장면 메타데이터](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#metadata).

이 메타데이터는 장면을 로컬에서 미리 보는 데는 모두 선택 사항이지만, 배포할 때는 일부가 필요합니다. 이 정보는 언제든지 수동으로 변경할 수 있습니다.

### package.json

이 파일은 NPM이 프로젝트를 식별하고 프로젝트의 종속성을 처리하는 데 필요한 정보를 제공합니다. Decentraland 장면에는 하나의 मुख्य 패키지가 필요합니다:

* **@dcl/sdk**: Decentraland SDK의 기본 종속성으로, 엔진, 컴포넌트, 시스템 등의 정의와 타입을 포함합니다.
* **@dcl/js-runtime**: @dcl/sdk와의 통합을 더 원활하게 해주는 일련의 타입 선언입니다.

장면에는 다른 패키지를 얼마든지 포함할 수 있습니다. 예를 들어 다음을 포함할 수 있습니다: [라이브러리](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 코드 작성이 더 쉬워지거나 특별한 기능을 사용할 수 있게 해주는 것들입니다.

### package-lock.json

이 파일에는 프로젝트의 다른 모든 종속성 버전이 나열됩니다. 이 버전은 고정되어 있으므로, 컴파일러는 여기에 나열된 것과 정확히 같은 마이너 릴리스를 사용합니다.

이 파일을 편집하여 어떤 패키지 버전이든 수동으로 변경할 수 있습니다.

### tsconfig.json

다음을 포함하는 디렉터리는 *tsconfig.json* 파일은 TypeScript 프로젝트의 루트 디렉터리입니다. 다음 *tsconfig.json* 파일은 프로젝트를 TypeScript에서 JavaScript로 컴파일하는 데 필요한 루트 파일과 옵션을 지정합니다.

장면에 추가 라이브러리를 설치하면 이 파일에 항목이 자동으로 추가되어야 합니다. Decentraland utils 라이브러리를 설치할 때는 이 파일을 수동으로 변경할 필요가 없어야 합니다.

## 권장 파일 위치

장면을 Decentraland에 배포할 때, 장면을 사용하는 데 필요한 모든 에셋이나 외부 라이브러리는 장면 폴더 안에 패키지되어 있거나 원격 서버를 통해 사용할 수 있어야 한다는 점을 기억하세요.

플레이어의 클라이언트에서 실행되어야 하는 모든 항목은 장면 폴더 안에 있어야 합니다. 로컬 컴퓨터의 다른 위치에 설치된 파일이나 라이브러리를 참조해서는 안 됩니다. 배포된 장면에서는 사용할 수 없기 때문입니다.

장면에 필요할 수 있는 다양한 유형의 에셋을 저장할 때는 다음 폴더 이름을 일관되게 사용하는 것을 권장합니다:

* 3D 모델: `assets/scene/models`
* 동영상: `assets/scene/videos`
* 사운드 파일: `assets/scene/sounds`
* 텍스처용 이미지 파일(glTF 모델 제외): `assets/scene/materials`
* *.ts* 컴포넌트 정의 `/src/components`
* *.ts* 시스템 정의 `/src/systems`


> [!WARNING]
> **📔 참고**: glTF 모델의 지원 파일, 예를 들어 텍스처 이미지 파일이나 *.bin* 파일은 항상 모델의 *.gltf* 또는 *.glb* 파일에 설명되어 있습니다.



> [!WARNING]
> **📔 참고**: 잠재적인 문제를 피하기 위해 모든 폴더와 파일 이름은 항상 소문자를 사용하는 것을 권장합니다.


Scene Editor를 통해 에셋을 가져오면 자동으로 다음 안에 추가됩니다: `assets/scene` 폴더. Scene Editor의 Asset Packs에 있는 기본 에셋을 사용할 경우, 해당 파일은 다음에 추가됩니다: `assets/builder` 폴더에.

## dclignore 파일

모든 장면에는 다음이 포함됩니다: *.dclignore* 파일이 있습니다. 이 파일은 장면을 Decentraland에 배포할 때 장면 폴더에서 어떤 파일을 무시할지 지정합니다.

예를 들어, 장면의 3D 모델용 Blender 파일을 장면 폴더 안에 보관하되, 해당 파일이 Decentraland에 배포되지 않도록 하고 싶을 수 있습니다. 이 경우 다음을 추가할 수 있습니다: `*.blend` 에서 *.dclignore* 를 추가하여 해당 확장자를 가진 모든 파일을 무시할 수 있습니다.


> [!WARNING]
> **📔 참고**: *.dclignore* 는 배포뿐 아니라 로컬 미리 보기에도 적용됩니다. 무시된 파일은 미리 보기가 제공하는 콘텐츠에 나열되지 않으므로, 장면에서도 불러올 수 없습니다. 로그에서 확인할 수 있는 404 오류가 아니라, 파일이 아예 요청되지 않습니다. 에셋은 에디터에서는 작동하는데 미리 보기에서는 작동하지 않는다면, 먼저 다음을 확인하세요: *.dclignore* 먼저.


| 무엇을 무시할지 | 예시          | 설명                                                |
| -------- | ----------- | ------------------------------------------------- |
| 특정 파일    | `BACKUP.ts` | 특정 파일을 무시합니다                                      |
| 폴더       | `drafts/`   | 폴더와 그 하위 폴더의 전체 내용을 무시합니다                         |
| 확장자      | `*.blend`   | 지정한 확장자를 가진 모든 파일을 무시합니다                          |
| 이름 섹션    | `test*`     | 쿼리와 일치하는 이름의 모든 파일을 무시합니다. 이 경우, 다음으로 시작하는 *test* |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/kinds-of-projects/scene-files.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
