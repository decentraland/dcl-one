> 다른 사람들과 공유할 수 있도록 자신만의 Decentraland 라이브러리를 만드세요

# 라이브러리 만들기

라이브러리는 흔한 문제에 대한 해결책을 공유하는 훌륭한 방법입니다. 복잡한 과제는 한 번만 접근해서 해결책을 라이브러리에 캡슐화해 두면, 다시 같은 문제가 생길 때는 코드 한 줄만 작성하면 됩니다. 커뮤니티와 라이브러리를 공유하면 모든 창작자의 생산성을 기하급수적으로 높일 수 있습니다.

현재, 이 라이브러리들은 [Examples 페이지에](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 모두 사용할 수 있습니다. 여러분도 직접 만들어 공유하길 권장합니다.

여기에 자세히 설명된 단계를 따르면, Decentraland 씬과 호환되고 다른 사람들과 쉽게 공유할 수 있는 라이브러리를 만들 때 따르는 대부분의 복잡성을 피할 수 있습니다.

## 라이브러리 만들기

> **📔 참고**: 라이브러리를 빌드하기 전에 Node 버전 20 이상을 사용하고 있는지 확인하세요.

자신만의 라이브러리를 만들고 NPM을 통해 공유하려면 다음을 수행하세요:

1. 새 빈 폴더를 만들고, 그곳에서 새 VS Studio Code 창을 여세요.
2. Visual Studio의 왼쪽 여백 사이드바에서 Decentraland 탭을 선택하세요
3. 클릭 **프로젝트 만들기**, 다음을 선택하세요 **라이브러리**

   이렇게 하면 Decentraland 라이브러리에 필요한 모든 기본 파일과 종속성이 생성됩니다.
4. 고유한 `이름` 을 `package.json`에 설정하세요. 이 이름은 NPM에 게시할 때 사용됩니다. npm에서 이미 같은 이름을 사용하는 기존 프로젝트가 없는지 확인하세요. 또한 다른 사람들이 여러분의 라이브러리를 찾을 수 있도록 설명과 태그도 추가하세요.
5. 새 *공개* GitHub 저장소를 프로젝트용으로 만드세요.

   이 프로젝트는 github actions를 사용하도록 구성되어 있어, main에 푸시할 때마다 패키지의 새 버전을 게시합니다. `main`.

   in the `package.json` 파일에서, repository에 `저장소` 에서 `url` 필드가 저장소의 URL을 가리키도록 하세요. 그러면 [npmjs.com](https://www.npmjs.com).
6. NPM 토큰 받기:
   1. <https://www.npmjs.com/에서> 계정을 만들거나 로그인하세요.
   2. 다음으로 이동하세요 **Account > Access Tokens > Generate new Token** 새 토큰을 만들고, 여기에 **게시** 권한을 부여하세요.

      성공 메시지에는 새로 생성된 토큰의 문자열이 포함됩니다. 이 문자열을 복사해 안전한 곳에 보관하세요.
7. GitHub 저장소에서 다음으로 이동하세요 **Settings > Secrets > Actions**, 그런 다음 **새 저장소 비밀**.

   비밀 이름을 **NPM\_TOKEN**로 지정하고, NPM 토큰의 문자열을 값으로 붙여넣으세요.
8. 변경 사항(아무 변경 사항이나)을 GitHub 저장소의 main 브랜치에 푸시하면 패키지가 게시됩니다.

   이것으로 끝입니다. 이제 누구나 다음을 사용해 패키지를 설치할 수 있습니다 `npm i <패키지-이름>`! 이제 이름으로 검색하면 라이브러리를 찾을 수 있습니다 [npmjs.com](https://www.npmjs.com).
9. 채우세요 `/src` 폴더를 여러분이 노출하고 싶은 모든 기능으로 채우고 GitHub에 변경 사항을 푸시하세요.

## 개발

라이브러리의 `index.ts` 파일에 노출하고 싶은 함수, 컴포넌트 등을 추가하면 가져오기 쉬워집니다.

예를 들어, 다음과 같은 `RotatorComponent` 컴포넌트를 `index.ts`추가하면, 다음과 같이 작성해 이 컴포넌트를 씬으로 가져올 수 있습니다.:

`import { RotatorComponent } from 'my-library'`

개발하는 동안 새 Decentraland 씬에서 사용해 보며 라이브러리를 테스트하세요. 다음을 실행해 npm 패키지를 씬에 설치하세요 `npm i <패키지-이름>`. 그런 다음 라이브러리의 기능을 시험해 볼 수 있는 씬을 만드세요. 라이브러리가 모든 경우에 예상대로 동작하는지 확인하기 위해, 씬으로 다양한 테스트 케이스를 다뤄 보세요.

### 빠른 반복

라이브러리에 계속해서 작은 조정을 하고 테스트해야 한다면, 변경 사항을 커밋한 뒤 씬에서 새 버전을 시험하기 전에 npm 게시가 완료될 때까지 기다려야 하는 일이 지칠 수 있습니다. 다행히도, 라이브러리로 테스트를 실행하는 훨씬 빠른 대안이 있습니다.

다음과 같은 **테스트 씬 프로젝트에서**:

1. 이 스크립트를 `scripts` 목록에 package.json에 추가하세요: `"link-sdk": "cd node_modules/@dcl/sdk && npm link && cd ../js-runtime && npm link"`
2. 실행하세요 `npm install`
3. 실행하세요 `npm install YOUR_LIBRARY_PATH`, 예: `npm install /User/projects/dcl-sdk7-library-test`
4. 실행하세요 `npm run link-sdk`

in the **라이브러리 프로젝트**:

1. 실행하세요 `npm install`
2. 실행하세요 `npm run build`
3. 실행: `npm link @dcl/sdk @dcl/js-runtime`

> **❗경고**: 이 단계들의 순서는 중요합니다. 다른 순서로는 작동하지 않을 수 있습니다.

이렇게 하면 씬이 로컬 드라이브에 직접 있는 라이브러리 버전과 동기화된 상태로 유지됩니다. 테스트하고 싶은 라이브러리 변경 사항은 라이브러리 폴더에서 `npm run build` 를 실행하기만 하면 되며, 변경 사항을 GitHub나 NPM에 게시할 필요가 없습니다.

> **💡 팁**: 연결이 성공했는지 확인하려면 `npm ls --link`를 실행하세요. 로컬 파일의 폴더를 가리키는 라이브러리 이름이 보여야 합니다.

라이브러리를 변경했다면, 업데이트하려면 `npm run build` 를 실행해야 합니다. 매번 그렇게 하지 않으려면:

1. 라이브러리에 "start": "tsc -p tsconfig.json --watch" 스크립트를 추가하세요
2. 실행하세요 `npm start` 를 라이브러리에서

테스트를 마치면 라이브러리 연결을 해제하는 것을 잊지 마세요.

1. 씬 폴더에서 `npm install <라이브러리 이름>`
2. 그런 다음 라이브러리에서 실행하세요 `rm -rf node_modules && npm install`

## 버전 관리

라이브러리 버전은 자동으로 다음에 게시됩니다 `npm` 에 `@latest` 와 `@next` 플래그와 함께.

해당 `@next` 플래그는 항상 `main` 브랜치의 마지막 커밋을 가리킵니다. 마지막 변경 사항이 테스트되지 않았을 수 있으므로 이 버전은 불안정할 수 있습니다. 라이브러리 사용자는 다음을 수행해(자신의 책임하에) 설치할 수 있습니다 `npm i <라이브러리 이름>@next`.

해당 `@latest` 플래그는 라이브러리의 마지막 안정 릴리스를 가리킵니다. 이것이 라이브러리 사용자가 보통 설치해야 하는 버전입니다. npm이 다음을 수행할 때 가져오는 버전입니다 `npm i <라이브러리 이름>`.

를 실행할 때 가져오는 버전입니다 `@latest` 플래그가 최신 커밋을 가리키게 하려면 **릴리스** 를 GitHub에서 만들어야 합니다.

1. 프로젝트의 GitHub 페이지를 엽니다. 페이지 오른쪽 여백에 있는 **Releases** 링크를 여세요.
2. 클릭 **새 릴리스 초안**.
3. 에서 **태그 선택** 새 버전 이름을 쓰세요. 예를 들어 "1.1.0"입니다. 또한 **Release Title**에도 이름을 쓰세요. 보통 같은 이름인 "1.1.0"입니다.

   > 중요: 태그는 숫자여야 하며, 추가 글자나 기호가 없어야 합니다. 숫자는 이전에 게시된 릴리스보다 커야 합니다.
4. 사용자에게 새로운 내용을 알 수 있도록 릴리스를 설명하세요.

   > 팁: 버튼을 클릭하세요 **자동으로 릴리스 노트 생성** 를 눌러 마지막 릴리스 이후의 모든 커밋을 출력하세요.
5. 누르세요 **릴리스 게시**. 이 작업은 `@latest` 플래그와 함께 npm으로 새 자동 게시를 트리거합니다. 이제 라이브러리 사용자는 이 버전을 다운로드하게 됩니다.

## 사용성에 대한 참고사항

다른 창작자도 쉽게 사용할 수 있도록 라이브러리를 만드는 데 최선을 다하세요. 우리의 가정은 우리가 만드는 도구의 형태를 항상 결정하는 데 도움이 됩니다. 종종 이러한 가정은 우리에게는 명백해 보이지만, 다른 맥락을 가진 사람들에게는 완전히 다른 가정일 수 있습니다.

* 함수, 컴포넌트, 매개변수를 포함해 라이브러리의 모든 것에 신중하게 이름을 지으세요. 완전히 수수께끼 같은 짧은 이름보다, 길더라도 스스로 설명되는 이름이 더 낫습니다.
* 라이브러리의 다양한 가능한 사용 사례를 폭넓게 생각해 보세요. 누군가는 스마트 웨어러블에서 라이브러리를 사용할 수도 있고, 누군가는 시스템을 때때로 켜거나 꺼야 할 수도 있으며, 누군가는 같은 씬에 시스템의 여러 인스턴스를 추가해야 할 수도 있습니다. 모든 사람이 여러분과 같은 어려움에 직면하는 것은 아닙니다.
* 그렇다고 해서 가능한 모든 사용 사례를 지원할 필요는 없습니다. 사실 유연성과 사용의 단순성 사이에는 언제나 절충이 있습니다. 좋은 도구를 만드는 것은 올바른 균형을 찾는 일입니다. 지원하지 않기로 한 범위의 경계를 어디에 둘지 전략적으로 정하세요. 다만, 선택한 제한 사항은 명확하게 문서화하는 것이 중요합니다.
* 이제 문서화입니다. 예제, 각 매개변수가 하는 일에 대한 설명, 어떤 시나리오는 동작하지 않는지에 대한 설명, 그리고 사용되는 맥락에 대해 어떤 가정을 하고 있는지에 대한 코멘트와 함께 라이브러리를 명확하게 문서화하세요. 라이브러리의 README.md는 이러한 내용을 공개하기에 종종 가장 좋은 장소입니다.
* 또 다른 훌륭한 도구는 메타데이터 코멘트를 코드 안에 직접 추가하는 것입니다. 이러한 코멘트는 라이브러리 사용자가 입력할 때 IDE(예: VS Studio Code)에 표시됩니다. 각 함수/컴포넌트의 용도, 각 매개변수가 어떤 값을 받아야 하는지, 함수가 무엇을 반환하는지 설명할 수 있습니다. 덕분에 라이브러리 사용자는 코드와 문서를 오갈 필요가 없고, 모든 것이 한곳에 있습니다! 예를 들어, 다음 함수는 `@dcl/ecs-scene-utils` 라이브러리의 것입니다:

  ```ts
  /**
   * 값이 최소값이나 최대값을 넘지 않도록 제한합니다.
   *
   * @param value - 입력 숫자
   * @param min - 최소 출력값.
   * @param max - 최대 출력값.
   * @returns 최소값과 최대값 사이의 결과 매핑된 값
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

  이 라이브러리의 사용자는 `clamp()` 함수를 입력할 때 이런 힌트를 보게 됩니다:
* 항상 타입을 선언하세요. 사람들이 어떤 객체 구조를 사용해야 하는지 추측하게 두지 마세요.
* 라이브러리는 가볍고 하나의 기능에 집중하도록 유지하세요. Decentraland 씬이 컴파일될 때, 해당 씬의 모든 라이브러리에 있는 모든 코드가 패키징되며, 씬에서 한 번도 호출되지 않는 코드까지 포함됩니다. 이런 이유로 크고 무거운 라이브러리를 만드는 것은 피하는 것이 좋습니다. 창작자가 정말 필요한 것만 사용할 수 있게 해 주는 가벼운 라이브러리가 더 낫습니다.
* 라이브러리 저장소를 주시하세요. 사람들이 풀 리퀘스트를 보내거나 이슈를 보고할 수 있습니다.
* 저장소에 라이선스 정보를 포함하세요. 그러면 다른 사람들이 여러분의 라이브러리를 자유롭게 사용할 수 있는지 알 수 있습니다. 기본 라이브러리에는 오픈 Apache 2 라이선스가 포함되어 있으며, 원하시면 언제든 바꿔도 됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/libraries/create-libraries.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
