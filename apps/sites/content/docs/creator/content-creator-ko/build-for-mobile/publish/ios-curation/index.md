> iOS 콘텐츠 큐레이션 방식과 씬 또는 월드를 검토용으로 제출하는 방법.

# iOS 큐레이션

다음을 준수하기 위해 [Apple의 사용자 생성 콘텐츠(UGC) App Store 검토 가이드라인을](https://developer.apple.com/app-store/review/guidelines/#user-generated-content)Decentraland는 **수동 검토 절차** 를 도입했습니다. 이는 **iOS 앱의 Discover 섹션에 표시되는 모든 장면과 월드에 적용됩니다.**.


> [!NOTE]
> 이 검토 절차는 iOS에만 적용됩니다. Android의 Discover 섹션과 검색은 계속해서 모든 게시된 콘텐츠를 제한 없이 표시합니다.


## 작동 방식

* 모든 콘텐츠는 iOS 앱의 Discover 섹션에 표시되기 전에 수동으로 검토되어야 합니다.
* 승인되지 않은 장면과 월드는 iOS Discover 섹션에서 숨겨집니다.
* 승인되지 않은 장면이나 월드와 연결된 이벤트도 iOS Discover 섹션에서 숨겨집니다.
* **모든 장면은 계속 접근 가능하며** iOS에서 딥 링크 또는 `/goto` 및 `/world` 채팅 명령어를 통해 언제든지 접근할 수 있습니다. — 큐레이션은 Discover 표시 여부에만 영향을 줍니다.

## iOS 검토를 위해 장면 또는 월드를 제출하세요

장면 또는 월드를 iOS Discover 섹션에 포함되도록 검토받고 싶다면, **콘텐츠 검토 요청** 을 Decentraland Discord 서버에 제출하세요:

1. 다음에 참여하세요: [Decentraland Discord](https://dcl.gg/discord).
2. 다음으로 이동하세요: **#rl-mobile-curation** 채널.
3. 다음을 포함하여 요청을 게시하세요:
   * 장면 또는 월드 이름
   * 좌표 또는 월드 이름(예: `0,0` 또는 `MyWorld.dcl.eth`)
   * 콘텐츠에 대한 간단한 설명

제출물은 **24시간** 이내에 검토됩니다. 콘텐츠가 승인되면 또는 추가 조치가 필요한 경우 채널에서 알림을 받게 됩니다.

## 관련

* [추천받기](/docs/creator/content-creator-ko/build-for-mobile/publish/get-featured/)
* [모바일 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/)
* [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/publish/ios-curation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
