> Decentraland 팀이 제작한 오픈소스 씬으로, 모바일 플레이에 최적화되어 있습니다.

# 샘플 씬

다음 씬들은 완전한 오픈소스이며 모바일에서도 잘 작동하도록 설계되었습니다. 구조, 입력 처리, 성능 패턴의 참고 자료로 활용하세요.

## 데드 서지

멀티플레이어 좀비 슈팅 게임.

* **월드:** `/deadsurge.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/dead-surge](https://github.com/dcl-regenesislabs/dead-surge)

## 코지 팜

농장 시뮬레이터.

* **월드:** `/cozyfarm.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/cozy-farm](https://github.com/dcl-regenesislabs/cozy-farm)

## 킥 오프

월드컵 예측 게임.

* **월드:** `/kickoff.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/kickoff-2026](https://github.com/dcl-regenesislabs/kickoff-2026)

## 뗏목 게임

싱글플레이어 생존 게임.

* **월드:** `/raft.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/raft-game](https://github.com/dcl-regenesislabs/raft-game)

## 베네치안 헌트

멀티플레이어 오브젝트 헌트.

* **월드:** `/venetianhunt.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/venetian-hunt](https://github.com/dcl-regenesislabs/venetian-hunt)

## 광기의 탑

멀티플레이어 파쿠르 / 점프 업.

* **월드:** `/towerofmadness.dcl.eth`
* **소스:** [github.com/dcl-regenesislabs/towerofmadness](https://github.com/dcl-regenesislabs/towerofmadness)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/mobile-client/sample-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
