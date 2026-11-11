> 보안 권장 사항

# 보안

<details>

<summary>내 지갑 주소를 공유해도 되나요?</summary>

네, 지갑 주소는 숫자와 문자의 고유한 문자열로, 은행 계좌번호와 같습니다. 다른 사람들은 이를 사용해 웨어러블 같은 선물을 보낼 수 있지만, 당신의 아이템을 가져갈 수는 없습니다.

Decentraland 계정의 지갑 주소는 인월드에서 프로필을 클릭하거나 decentraland.org에 로그인한 상태에서 확인할 수 있습니다. 아바타 이름 아래에 표시됩니다.

</details>

<details>

<summary>누군가가 저에게 아이템을 무료로 보내고 싶다는데, 사기인가요?</summary>

웨어러블이나 이모트 선물을 보내는 것은 Decentraland에서 흔한 관행이므로, 이것이 자동으로 위험 신호로 간주되지는 않아야 합니다. 누군가가 당신에게 선물을 보내려면 숫자와 문자의 고유한 문자열인 지갑 주소가 필요하지만, 이 정보만으로는 당신을 속이는 데 사용할 수 없습니다. 누군가 당신의 지갑 주소를 사용해 선물을 보내면, 당신이 별도의 조치를 취하지 않아도 백팩에 나타나야 합니다.

당신이 해야 할 것은 **절대** 공유해서는 안 되는 것은 '시드 구문'입니다(이 내용은 Decentraland 계정이 외부 디지털 지갑에 연결된 경우에 해당합니다).

</details>

<details>

<summary>주의해야 할 가장 흔한 사기는 무엇인가요?</summary>

가장 흔한 사기 중 하나는 Decentraland를 사칭하며 MANA 무료 에어드롭을 광고하는 웹사이트, 소셜 계정, 이메일, DM입니다. 이는 Decentraland가 **절대** 하지 않습니다.

또 다른 흔한 수법은 잠재 고객인 척하며 바이러스가 들어 있는 파일을 보내는 것입니다. 개인적으로 알지 못하는 사람들의 메시지와 **절대** 그들이 보내는 파일을 다운로드할 때는 항상 매우 주의하세요.

![](/docs/_remote/dbcceacdfe273e0b51182b00.webp)

좋은 규칙은 어떤 조치를 취하기 전에 항상 시간을 들여 URL과 이메일 주소를 확인하는 것입니다. 진짜 Decentraland 웹사이트는 항상 기본 [decentraland.org](http://decentraland.org) 그리고 주간 뉴스레터는 <hello@decentraland.org>에서만 발송됩니다.

자세히 알아보기 [보안 팁과 요령](https://decentraland.org/blog/about-decentraland/how-to-keep-your-digital-assets-safe-in-the-metaverse).

</details>

<details>

<summary>내 계정/디지털 지갑이 해킹당했다면 어떻게 해야 하나요?</summary>

안타깝게도 계정 침해 피해자가 되어 지갑이 비어 있는 것을 발견했다면, 해킹이 어떻게 발생했는지에 따라 다음에 해야 할 몇 가지 조치가 있습니다:

* 지갑에 남아 있는 자산이 있다면 즉시 새롭고 침해되지 않은 계정으로 옮겨 보세요. 그리고 침해된 지갑은 완전히 사용을 중단하세요. 컴퓨터에 바이러스가 아직 남아 있을 경우를 대비해 다른 기기에서 새 지갑을 만드세요.
* 해킹이 다운로드한 파일을 통해 발생했다면, 악성 코드를 제거하기 위해 PC를 완전히 초기화해야 합니다.
* 해킹이 어떻게 발생했든, 해당 사건을 지역 당국에 신고할 수 있습니다. 이는 현지 법규에 따라 조사에 도움이 될 뿐만 아니라 경찰 보고서를 작성하는 데도 도움이 되며, Opensea 같은 플랫폼에서 침해된 자산을 표시하는 데 유용할 수 있습니다.

자산을 안전하게 보호하고 기기가 깨끗한지 확인한 후에는 새 Decentraland 계정을 만들어 다시 탐험을 시작할 수 있습니다.

</details>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/faqs/security.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
