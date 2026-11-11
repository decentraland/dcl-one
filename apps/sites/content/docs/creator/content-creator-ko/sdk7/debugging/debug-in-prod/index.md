> Decentraland 내부에서 실행 중인 씬을 디버그하는 방법

# 운영 중 디버그

이미 Decentraland에 배포된 씬을 실행할 때, 문제를 디버깅하기 위해 시도해 볼 수 있는 몇 가지 방법이 있습니다.

## 배포하기 전에

### 미리보기

씬을 Decentraland에 배포하기 전에, 최신 버전의 Decentraland SDK를 사용해 미리보기에서 씬이 잘 실행되는지 확인하세요. 참고: [미리보기에서 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-preview/).

## 빠른 다시 로드

지금 서 있는 씬을 다시 로드해야 한다면, 채팅창에 다음을 입력하고 Enter를 누르세요:

`/reload`

## 게시 파이프라인 소개

게시할 때마다 내부 프로세스가 렌더링되기 전에 모든 3D 모델을 최적화한다는 점을 기억하세요. 보통 몇 초면 끝나지만, 아주 큰 씬이거나 서버가 바쁠 때는 더 오래 걸릴 수 있습니다. 이 작업이 끝날 때까지 플레이어에게는 씬의 이전 버전이 제공됩니다.

다음을 사용해 씬에 대한 이 프로세스의 현재 상태를 확인할 수 있습니다 [이 도구](https://decentraland.github.io/opscli/). 변환이 완료되었다면, 자산의 세 가지 버전 모두에 초록불이 켜져 있어야 합니다.

## 멀티플레이어 서버 로그 및 메트릭

배포된 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 씬의 경우, 다음을 사용하세요 [씬 인사이트](/docs/creator/content-creator-ko/sdk7/debugging/scene-insights/) 지난 7일간의 서버 로그를 검색하고, 새 로그 줄을 실시간으로 추적하며, CPU 및 메모리 메트릭을 확인할 수 있습니다. 지갑 로그인과, 당신이나 AI 어시스턴트가 씬 프로젝트에서 실행할 수 있는 명령도 포함됩니다.

## 씬 로그

일반적으로 Decentraland를 사용할 때는 디버그 메시지를 확인하기 위해 콘솔을 열 수 없습니다. 콘솔을 사용할 수 있게 하려면, 다음 옵션으로 Decentraland를 열어야 합니다 `scene-console` 매개변수입니다. 그런 다음 키보드의 백틱 키를 눌러 콘솔을 전환할 수 있습니다: **\`**. 이 키는 대부분의 영어 키보드에서 1 키의 왼쪽에 있습니다.

다음 옵션으로 Decentraland를 열려면 `scene-console` 매개변수를 사용하여, 다음 중 하나를 수행하세요:

* 브라우저 창에 다음 딥 링크를 입력하세요: `decentraland://?position=0,0&scene-console=true`. 설치되어 있다면 Decentraland 데스크톱 애플리케이션이 열립니다.
* 명령줄에 다음을 입력하세요:
  * **macOS**: `open Decentraland.app --args --position 0,0 --scene-console true`
  * **Windows**: `"C:\Users\[YOUR-USER]\Downloads\Decentraland_windows64\Decentraland.exe" --position 0,0 --scene-console true`


> [!NOTE]
> **💡 팁**: 다음을 변경하세요 **위치** 매개변수를 씬의 좌표로 변경하면, 씬으로 바로 로드됩니다.


다음 옵션으로 Decentraland를 실행하면 `scene-console`, 콘솔을 여는 방법은 세 가지입니다:

* 다음을 클릭하세요 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 오른쪽 상단 모서리의 아이콘
* 다음 키를 누르세요 **\`** 짧은 콘솔을 위한 키
* Shift + 를 누르세요 **\`** 콘솔의 더 큰 보기를 열려면

활성화된 각 씬의 메시지가 콘솔에 기록되므로, 콘솔에 표시되는 내용 중 일부는 씬과 관련이 없을 수 있습니다.

## 버그 신고

씬 자체의 문제가 아니라 Decentraland SDK 전반에 관한 문제를 발견했다면, 다음을 참조하세요 [버그 신고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/debugging/debug-in-prod.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
