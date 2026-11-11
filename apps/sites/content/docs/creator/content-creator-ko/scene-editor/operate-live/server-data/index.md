> 씬의 멀티플레이어 서버에 저장된 라이브 데이터를 보고 편집합니다.

# 서버 데이터

하나를 사용하는 씬 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 서버에 데이터를 저장할 수 있습니다: 순위표, 플레이어 진행 상황, 문이 열리거나 아이템이 배치되는 등의 환경 변경 사항 등. 이 데이터는 업데이트됩니다 **실시간으로** 플레이어가 게시한 씬과 상호작용할 때.

웹 UI에서 이 모든 데이터를 볼 수 있으며, 또한 **그곳에서 직접 변경할 수도 있습니다**. 변경한 내용은 다시 게시할 필요 없이 라이브 씬에 바로 적용됩니다. 여기에는 다음이 포함됩니다:

* **씬 데이터**: 순위표나 문의 상태처럼 모든 플레이어가 공유하는 값입니다.
* **플레이어 데이터**: 진행 상황이나 환경 설정처럼 단일 플레이어를 위해 저장되는 값입니다.
* **환경 변수**: 서버만 읽을 수 있는 설정 값과 비밀 정보입니다.

## 저장소 UI에 액세스

저장소 UI를 다음에서 직접 열 수 있습니다 [decentraland.org/storage](https://decentraland.org/storage).

Creator Hub에서도 접근할 수 있습니다:

1. 다음을 엽니다 **관리** Creator Hub의 섹션.
2. 다음을 클릭하세요: **점 세 개** 게시한 콘텐츠가 있는 장소 옆에 있는
3. 선택 **저장소 보기**.

<img src="/docs/_remote/512f80176ec141582be43e70.webp" alt="Creator Hub의 저장소 보기 옵션" width="400">

저장소 UI에는 씬을 게시했거나 운영자 권한이 있는 모든 월드와 LAND 위치의 목록이 표시됩니다. 씬을 열면 데이터가 세 개의 탭으로 정리되어 표시됩니다: **씬**, **플레이어**, 그리고 **환경**.


> [!NOTE]
> **💡 팁**: 이 데이터는 멀티플레이어 서버를 사용하고 다음을 통해 데이터를 저장하는 씬에만 존재합니다 `저장소` API입니다. 다음을 참조하세요 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 설정 방법을 알아보려면


## 씬 데이터

해당 **씬** 탭에는 모든 플레이어가 공유하는 저장된 변수들이 모두 나열됩니다. 예를 들어 순위표나 현재 환경 상태가 있습니다.

![Scene data tab](/docs/_remote/76bca9abaef9d73bc207163d.webp)

이 목록의 값은 플레이어가 씬과 상호작용하는 동안 실시간으로 업데이트됩니다. 연필 또는 휴지통 아이콘을 클릭해 이 변수들을 편집하거나 삭제할 수도 있습니다. 변경 사항은 실행 중인 씬에 반영되므로, 순위표를 초기화하는 것처럼 라이브 값을 조정할 때 다시 게시하지 않고도 유용하게 사용할 수 있습니다.

## 플레이어 데이터

해당 **플레이어** 탭에는 서버에 어떤 데이터든 저장된 모든 플레이어가 나열됩니다. 지갑 주소나 이름으로 플레이어를 검색한 다음, 해당 플레이어와 연결된 모든 데이터를 볼 수 있습니다.

씬 데이터와 마찬가지로 연필 또는 휴지통 아이콘을 클릭해 어떤 값이든 편집하거나 삭제할 수 있습니다.

이는 지원에 특히 유용합니다. 특정 플레이어가 씬에서 문제를 신고하면 그 플레이어를 찾아 저장된 데이터를 검토해 상황을 파악할 수 있습니다. 예를 들어 씬의 이전 버전에서 나온 상충되는 데이터 때문에 좋지 않은 상태에 갇히게 되었다면, 아무것도 다시 배포하지 않고도 해당 기록을 편집하거나 지워 안정적인 상태로 복구할 수 있습니다.

## 환경 변수

해당 **환경** 탭에는 씬에 대해 정의된 모든 환경 변수가 나열됩니다. 이들은 서버만 읽을 수 있는 설정 값이므로 다음과 같은 용도로 적합합니다 **비밀 정보** 예를 들어 개인 키나 보상 수령 코드 같은 것들입니다. 이러한 값은 플레이어의 기기를 거치지 않으며, 씬의 게시된 코드에도 나타나지 않습니다.

![Environment variables tab](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

이 탭에서 새 변수를 추가하거나, 연필 또는 휴지통 아이콘을 사용해 기존 변수를 덮어쓰거나 삭제할 수 있습니다. 단, *읽을* 변수의 현재 값을 읽을 수는 없습니다. 이는 민감한 데이터를 보호하기 위한 의도적인 동작입니다. 값은 대체하거나 삭제만 할 수 있습니다.

환경 변수는 경기 시간이나 최대 플레이어 수 같은 기능 플래그나 게임 매개변수에도 매우 유용하며, 이를 다시 게시하지 않고도 라이브 씬에서 조정하고 싶을 수 있습니다.

## 자세히 알아보기

씬 코드에서 이 데이터를 읽고 쓰는 방법과, 시간이 지나면서 저장된 데이터의 구조를 변경할 때의 모범 사례를 알아보려면 다음을 참조하세요 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/operate-live/server-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
