> Web Editor에서 씬을 Creator Hub로 마이그레이션합니다.

# Creator Hub로 마이그레이션

Creator Hub이 아닌 다른 도구로 만든 장면이 있다면, 쉽게 Creator Hub로 마이그레이션할 수 있습니다.

Creator Hub은 Decentraland 장면을 만드는 데 권장되는 도구입니다. Web Editor보다 훨씬 세련된 인터페이스를 제공하며, 쉬운 드래그 앤 드롭 인터페이스와 코드를 통한 추가 맞춤 설정 기능을 함께 사용할 수 있습니다. 또한 최신 데스크톱 클라이언트를 사용하여 장면 미리보기를 실행할 수 있습니다.

## Web Editor에서 마이그레이션하기

Web Editor로 만든 장면의 코드를 편집하려면, 장면을 자신의 컴퓨터로 내보내고 Creator Hub으로 열어야 합니다.


> [!WARNING]
> **📔 참고**: Creator Hub이 설치되어 있지 않다면, 시작하기 전에 다음 페이지의 단계를 따르세요.
> 
> [Creator Hub 설치](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/)


1. 다음을 클릭하세요 **다운로드 아이콘** 장면을 편집하는 동안 Web Editor 상단 메뉴에서.

![](/docs/_remote/013ac41c9f6c08ef06e89d47.webp)

2. 이렇게 하면 다음 항목이 다운로드됩니다. *.zip* 파일을 다운로드한 뒤 압축을 푸세요.
3. 을 엽니다. **Creator Hub**, 다음으로 이동하세요 **Scenes** 섹션.
4. 다음을 클릭하세요 **장면 가져오기** 버튼을 누른 뒤 내보낸 프로젝트 폴더의 경로를 선택하세요.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

작업이 끝나면 Creator Hub 안에서 프로젝트 작업을 계속할 수 있습니다. 시각적 인터페이스는 Web Editor와 매우 비슷하지만 훨씬 더 세련되어 있습니다.

또한 `/src` 폴더의 파일을 편집하여 코드로 장면에 동작을 추가할 수도 있습니다.  [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) 장면의 코드를 편집하는 방법은 다음을 참조하세요.

## 코드 전용 프로젝트 마이그레이션하기

코드 전용 프로젝트라면 무엇이든 Creator Hub으로 가져올 수 있습니다. 이렇게 하려면,

1. Creator Hub을 열고 다음으로 이동하세요. **Scenes** 섹션.
2. 다음을 클릭하세요 **장면 가져오기** 버튼을 누른 뒤 내보낸 프로젝트 폴더의 경로를 선택하세요.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

작업이 끝나면 Creator Hub 안에서 프로젝트 작업을 시작할 수 있습니다. 이렇게 해도 계속해서 즐겨 사용하는 코드 편집기로 장면의 코드를 편집하거나, 명령줄을 사용해 장면을 실행하거나 배포하는 것을 막지는 않습니다.

프로젝트를 가져오면 코드로 생성된 모든 콘텐츠는 Creator Hub 캔버스에서 보이지 않거나 편집할 수 없게 되어, 새 항목을 배치하고 정렬하기가 어려울 수 있습니다. 처음에는 장면이 빈 격자로 표시됩니다.

![](/docs/_remote/3b0a67be118d41cad070cf4b.webp)

콘텐츠를 처음부터 캔버스에 수동으로 추가하는 대신, 명령을 실행하여 자동으로 추가할 수 있습니다. 이렇게 하려면 SDK의 최신 버전이 설치되어 있는지 확인한 다음, 터미널에서 다음 명령을 실행하세요:

```
npx sdk-commands code-to-composite
```


> [!CAUTION]
> **❗경고**: 이 명령을 실행하기 전에 프로젝트를 백업해 두세요.
> 
> 이 명령은 다음 파일을 덮어씁니다. `main.composite` 파일을 새 스냅샷으로 바꿉니다. 또한 다음 항목의 모든 코드를 주석 처리합니다. `.ts` 다음 폴더의 파일에 있는 `src` 폴더. 다시 실행하려면 코드의 주석을 해제해야 합니다.


이 명령은 장면을 실행하고 첫 번째 프레임에서 코드로 생성된 콘텐츠의 스냅샷을 찍습니다. 이 스냅샷은 다음 파일에 저장되며, `main.composite` 이 파일은 Creator Hub이 장면의 콘텐츠를 표시하는 데 사용합니다. 장면의 코드는 모든 엔티티가 중복 생성되는 것을 방지하기 위해 주석 처리됩니다.

이 명령은 Creator Hub UI에서 표현할 수 있는 엔티티와 컴포넌트만 캡처한다는 점에 유의하세요. 사용자 지정 컴포넌트나, 로직을 수행하는 코드, 또는 코드로 생성된 UI 요소는 복제하지 않습니다. 주석 처리된 동작을 다시 추가하려면 다음 위치의 코드를 편집해야 합니다. `.ts` 다음 폴더의 파일에 있는 `src` 폴더에서 필요한 줄의 주석을 해제하세요.

코드의 일부를 다시 작성하여 새 엔티티를 만드는 대신, 이름이나 태그로 기존 엔티티를 참조해 동작을 부여하고 싶을 수도 있습니다.  [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) 코드에서 이러한 엔티티를 가져오는 방법은 다음을 참조하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/get-started/migrate-from-web.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
