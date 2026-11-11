> iOS 및 Android용 Decentraland 모바일 앱 개요, 다운로드 링크, 씬을 모바일에 맞게 준비하기 위한 체크리스트.

# 개요

디센트럴랜드가 이제 모바일에서 이용 가능합니다. 플레이어는 데스크톱 클라이언트와 웹은 물론 iOS 및 Android 기기에서도 여러분의 씬을 탐험할 수 있습니다. 크리에이터로서 여러분은 터치 기반 기기에서 보기 좋고 잘 작동하도록 씬을 조정할 수 있습니다.

![디센트럴랜드 모바일 앱 — 제네시스 시티](/docs/_remote/a10f68e6dd27dfa895f53969.webp)



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/FBr2gye3qh8" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=FBr2gye3qh8>">Watch on YouTube</a></p>



## 모바일 앱 받기

* [iOS용 다운로드 (App Store)](https://apps.apple.com/app/decentraland/id6478403840?utm_source=docs&utm_medium=internal&utm_content=ios)
* [Android용 다운로드 (Google Play)](https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&pcampaignid=web_share&utm_source=docs&utm_medium=internal&utm_content=android)

## 모바일용 빌드

### 참고

* [샘플 씬](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/sample-scenes/) — 디센트럴랜드 팀이 제작하고 모바일 플레이에 최적화한 오픈 소스 씬입니다.
* [누락된 기능](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/missing-features/) — 데스크톱 클라이언트에서 사용할 수 있지만 아직 모바일 앱에서는 지원되지 않는 기능입니다.
* [하드웨어 요구 사항](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/hardware-requirements/) — 모바일에서 디센트럴랜드를 실행하기 위한 최소 및 권장 하드웨어 사양입니다.

### 개발

* [코드에서 플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) — 사용 `isMobile()` 하여 플랫폼별로 씬의 로직과 UI를 분기합니다.
* [모바일에서 씬 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) — Creator Hub 또는 CLI를 통해 휴대폰에서 직접 미리보세요.
* [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/) — 기기의 하드웨어 여백(노치, 홈 인디케이터)과 클라이언트 자체 컨트롤이 차지하는 영역입니다. UI가 배치되는 영역을 다음을 사용해 선택하세요 `screenInset`.
* [모바일 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/) — 해야 할 것과 하지 말아야 할 것, 크기 권장 사항 및 현재 제한 사항입니다.
* [모바일에서의 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/) — 터치 친화적인 입력 매핑과 어떤 `InputAction`것을 피해야 하는지.
* [성능 최적화](/docs/creator/content-creator-ko/build-for-mobile/develop/optimize-performance/) — 모바일 씬 제한, Creator Hub에서 이를 미리보는 방법, 그리고 성능 목표입니다.

### 게시

* [모바일 Discover에 추천되기](/docs/creator/content-creator-ko/build-for-mobile/publish/get-featured/) — 모바일 Discover 섹션에 대한 제출 요구 사항입니다.
* [iOS 선별](/docs/creator/content-creator-ko/build-for-mobile/publish/ios-curation/) — iOS 콘텐츠 선별이 어떻게 작동하는지, 그리고 검토를 위해 씬 또는 월드를 제출하는 방법입니다.

## 빠른 체크리스트

모바일에서 잘 작동해야 하는 씬을 게시하기 전에, 다음 사항을 확인하세요:

* [ ] 실제 기기에서 씬을 미리 보았습니다 — 다음을 참조하세요 [모바일에서 씬 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/).
* [ ] 모든 중요한 UI 요소가 [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/).
* [ ] 안에 유지됩니다 [UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/).
* [ ] 핵심 작업이 `IA_SECONDARY` 또는 `IA_ACTION_3`–`IA_ACTION_6` (데스크톱의 `F` 및 `1`/`2`/`3`/`4` 키)에 바인딩되어 있지 않습니다. 해당 버튼은 모바일에서 기본적으로 숨겨져 있으므로, 씬에서 표시하지 않는 한 — 다음을 참조하세요 [모바일에서의 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/).
* [ ] 여러분의 씬의 [성능](/docs/creator/content-creator-ko/build-for-mobile/develop/optimize-performance/) 은 중급형 휴대폰의 고화질 프로필에서 90%를 초과해야 합니다 — 다음을 참조하세요 [하드웨어 요구 사항](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/hardware-requirements/) 참고용 기기.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/mobile-client/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
