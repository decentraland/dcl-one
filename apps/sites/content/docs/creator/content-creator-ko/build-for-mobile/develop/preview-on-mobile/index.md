> Creator Hub 또는 CLI에서 실제 모바일 기기에서 씬을 미리 봅니다.

# 모바일에서 미리보기

Creator Hub 또는 명령줄에서 Decentraland 모바일 앱으로 씬을 직접 미리 볼 수 있습니다. 실제 기기에서 UI, 입력 처리, 성능이 제대로 동작하는지 확인할 수 있는 유일하게 신뢰할 수 있는 방법입니다.

## 사전 요구 사항

* 휴대폰에 Decentraland 모바일 앱이 설치되어 있어야 합니다 — [다운로드 링크](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/#get-the-mobile-app).
* 휴대폰과 개발 머신은 반드시 **같은 로컬 네트워크** (Wi‑Fi)에 연결되어 있어야 합니다. 미리보기는 컴퓨터에서 제공되며, QR 코드는 LAN URL로 연결됩니다.

## 옵션 A — Creator Hub에서

1. Creator Hub에서 씬을 엽니다.
2. 옆의 화살표를 클릭하여 **Play** 버튼을 열고 **Play 옵션**, 그런 다음 선택합니다. **모바일용 QR 코드 표시**.
3. 휴대폰 카메라로 표시된 QR 코드를 스캔합니다. 링크를 열면 Decentraland 모바일 앱이 열리고 씬 미리보기가 로드됩니다. 팝업 주위의 어두운 영역을 클릭하여 닫습니다.

<figure><img src="/docs/_remote/c6aa2bdbe9bdd432e1e7e96d.webp" alt="Creator Hub Play Options menu with the Show QR Code for Mobile option"><figcaption><p>Creator Hub의 Play 옵션 메뉴에 있는 "모바일용 QR 코드 표시" 옵션.</p></figcaption></figure>

## 옵션 B — 명령줄에서

씬 프로젝트의 루트에서 다음을 실행합니다:

```bash
npm run start -- --mobile
```

CLI는 씬의 LAN URL을 가리키는 QR 코드를 터미널에 출력합니다. 휴대폰으로 스캔하여 Decentraland 모바일 앱에서 씬을 로드하세요.

<figure><img src="/docs/_remote/ce46ee596043fea52191ae3d.webp" alt="QR code printed by the Decentraland CLI for mobile preview"><figcaption><p>. 휴대폰으로 스캔하여 Decentraland 모바일 앱에서 씬을 엽니다. <code>npm run start -- --mobile</code>. 휴대폰으로 스캔하여 Decentraland 모바일 앱에서 씬을 엽니다.</p></figcaption></figure>


> [!NOTE]
> **💡 팁**: 다음과 같이 전달하면 `--mobile`, 데스크톱 익스플로러도 함께 실행되지 않습니다. 둘 다 동시에 테스트하려면 `npm run start` 를 데스크톱용으로 하나의 터미널에서 실행하고 `npm run start -- --mobile` 를 모바일용으로 다른 터미널에서 실행하세요.


## 핫 리로드

데스크톱 미리보기와 마찬가지로, 씬 파일을 변경하면 모바일 미리보기도 자동으로 다시 로드됩니다. 편집할 때마다 QR 코드를 다시 스캔할 필요가 없습니다.

## 문제 해결

* **QR 코드가 앱을 열지 않음** — Decentraland 앱이 설치되어 있고 해당 기기에서 최소 한 번은 열어 보았는지 확인하세요.
* **휴대폰이 미리보기 서버에 연결되지 않음** — 휴대폰과 개발 머신이 같은 Wi‑Fi 네트워크에 있는지 확인하세요. 일부 기업 네트워크는 클라이언트를 격리하므로, 그런 경우 개인 핫스팟이나 다른 네트워크를 사용하세요.
* **씬이 휴대폰에서 로드되지만 데스크톱과 다르게 보임** — 이는 예상되는 동작입니다. 이 시점을 사용해 다음 항목을 검증하세요. [안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/), [UI 크기](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/#sizing), 그리고 [입력 바인딩](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/).

## 관련

* [씬 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/)
* [CLI 사용하기](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/)
* [모바일 Discover에 추천받기](/docs/creator/content-creator-ko/build-for-mobile/publish/get-featured/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
