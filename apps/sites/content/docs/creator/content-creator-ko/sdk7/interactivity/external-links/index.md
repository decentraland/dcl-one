> 다른 씬 또는 외부 사이트로 연결

# 외부 링크

장면에서 다른 콘텐츠로 링크를 추가할 수 있습니다. 다른 장면이나 외부 웹사이트로 연결할 수 있습니다.

## Creator Hub의 Scene Editor를 사용하세요

외부 링크나 텔레포트를 추가하는 가장 쉬운 방법은 Scene Editor를 사용하는 것입니다. 다음을 사용하세요 **텔레포트** [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) Genesis City의 다른 장면으로 텔레포트를 추가하려면, 또는 다음 중 아무거나 사용하세요 **소셜 링크** 스마트 아이템을 사용하여 외부 사이트로 연결되는 링크를 추가하세요.

![](/docs/_remote/71809788940cd245e36af334.webp)

## 텔레포트

플레이어를 다른 장면으로 텔레포트하려면, 플레이어를 텔레포트할 좌표를 지정하여 다음 함수를 호출하세요.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ worldCoordinates: { x: -51, y: 1 } })
```

플레이어는 텔레포트되기 전에 확인 화면을 보게 되며, 이 화면에는 대상 장면의 `scene.json 파일`, 장면을 포함한 `이름`, `설명` 및 `navmapThumbnail`로 설정하는 것입니다. 참조: [장면 메타데이터](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/) 이 데이터를 설정하는 방법에 대한 자세한 내용은

텔레포트는 지정된 좌표의 장면으로 이동시키지만, 반드시 그 정확한 좌표로 이동시키는 것은 아닙니다. 즉, 여러 파셀을 가진 장면으로 이동할 때 플레이어는 지정된 좌표와 정확히 같은 위치가 아니라, 해당 장면의 제작자가 지정한 스폰 지점 중 하나로 이동할 수 있습니다.

현재 장면 안에서 플레이어를 다른 좌표로 이동시키려면, 다음을 사용하세요 `movePlayerTo()` 함수 대신 사용하세요. 다음을 참조하세요 [플레이어 이동](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/move-player/).

## WORLD로 텔레포트

플레이어를 공개 Genesis City 지도에 게시되지 않은 장면으로 보내려면, 대신 분리된 [Decentraland WORLD](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds), 다음을 전달하세요 `realm` 을(를) `teleportTo()`.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ realm: 'mannakia.dcl.eth' })
```

`realm` 다음과 같은 월드 이름 또는 `mannakia.dcl.eth`, 또는 realm URL을 허용합니다.

다음이 없으면 `worldCoordinates`, 플레이어는 대상의 스폰 지점 중 하나에 도착합니다. 해당 월드 내의 특정 파셀을 선택하려면 좌표를 추가하세요:

```ts
// 특정 월드의 12,34 파셀에 도착
teleportTo({
	realm: 'mannakia.dcl.eth',
	worldCoordinates: { x: 12, y: 34 },
})
```

플레이어는 realm을 변경하기 전에 확인 화면을 보게 됩니다. 이 화면에서는 *"정말 이 월드에 들어가시겠습니까?"* 목적지(월드 이름 또는 realm URL의 호스트)를 보여줍니다. 플레이어는 수락해야만 이동됩니다.

월드에서 플레이어를 Genesis City로 다시 보내려면, 다음을 포함하지 마세요 `realm` 필드는 기본값이 Genesis City입니다.


> [!WARNING]
> **📔 참고**: `realm` 필드에는 다음이 필요합니다 `@dcl/sdk` 버전 7.28.0 이상.


### changeRealm은 더 이상 권장되지 않습니다

기존 장면에서는 `changeRealm()` 를 사용해 월드에 도달했습니다. 아직 작동하지만, 대신 다음을 사용하세요 `teleportTo({ realm })` 대신 사용하세요. `changeRealm()` 또한 확인 화면을 표시합니다. 해당 옵션의 `message` 필드는 기본 프롬프트 텍스트만 바꿉니다.

`changeRealm()` 플레이어가 프롬프트를 수락하는 즉시 완료되며, 새 realm이 실제로 활성화될 때가 아닙니다.  `teleportTo()` 그 직후의 호출은 **이전** realm을 대상으로 실행되어 플레이어를 잘못된 위치에 보내게 됩니다. `teleportTo({ realm })` 는 두 단계를 한 번의 호출로 수행하므로 그 간극이 존재하지 않습니다.

## 외부 링크

외부 웹사이트로 연결되는 링크를 추가하려면 다음을 사용하세요 `openExternalUrl()` 명령입니다.

```ts
import { openExternalUrl } from '~system/RestrictedActions'

openExternalUrl({ url: 'google.com' })
```

이 기능을 악용하여 플레이어에게 스팸을 보내는 것을 방지하기 위해, 다음을 호출할 수 있는 경우는 `openExternalUrl` 엔티티의 명시적인 클릭 또는 버튼 이벤트에서만 가능합니다. 타이머, 충돌 영역 또는 전역 클릭 이벤트의 결과로는 이 함수를 호출할 수 없습니다. 자세한 내용은 [버튼 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/) 를 참조하세요.

텍스처 사용이 `openExternalUrl` 가 호출되면 플레이어에게 확인 화면이 표시되며, 링크가 어디로 이동하는지 안내받고 링크 방문을 수락하거나 거부할 수 있습니다.

링크는 새 탭에서 열리며, 기존 탭은 Decentraland에 그대로 유지됩니다.

플레이어가 다음 항목을 체크하면 *이 도메인을 신뢰* 체크박스에 체크하면, 링크가 같은 장면에서 제공되고 같은 도메인으로 연결되는 한 세션 동안 다시 프롬프트가 표시되지 않습니다.

## 클립보드에 복사

문자열을 플레이어의 클립보드에 복사하려면 다음을 사용하세요 `copyToClipboard()`. 이후 플레이어가 *붙여넣기* 를 Decentraland 채팅이나 기기의 다른 애플리케이션에서 하면, 사용자는 당신의 문자열을 붙여넣게 됩니다.

```ts
import { copyToClipboard } from "~system/RestrictedActions"

copyToClipboard( { text: 'My text to copy' } )
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/external-links.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
