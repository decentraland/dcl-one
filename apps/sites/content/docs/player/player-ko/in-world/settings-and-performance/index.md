> Decentraland에서의 설정 및 성능

# 설정 및 성능

게임 내에 있을 때, 사이드바 메뉴에서 톱니바퀴 아이콘을 찾아 그래픽, 사운드, 컨트롤, 채팅 설정에 접근하세요.

## 기기에서 Decentraland의 성능 최적화

Decentraland의 오픈 월드를 렌더링하려면 일정 수준의 기술적 성능이 필요합니다. 아래에 나열된 최소 요구 사양을 충족하지 못하는 경우 성능 저하와 긴 로딩 시간을 경험할 수 있습니다. 이 경우 게임 내에서 그래픽 설정을 낮춰 Decentraland의 성능을 최적화해 볼 수 있습니다.

### **Windows**

|       | 최소 요구 사항                                                   | 권장 설정                                       |
| ----- | ---------------------------------------------------------- | ------------------------------------------- |
| 운영체제  | Windows 10 64비트                                            | Windows 11 64비트                             |
| CPU   | Intel i5 7세대 또는 AMD Ryzen 5 시리즈                            | Intel i7 12세대 또는 AMD Ryzen 7 시리즈            |
| GPU   | Nvidia RTX 20 시리즈 또는 AMD Radeon RX 5000 시리즈(DirectX 12 호환) | Nvidia RTX 30 시리즈 또는 AMD Radeon RX 6000 시리즈 |
| VRAM  | 6 GB                                                       | 12 GB                                       |
| RAM   | 16GB                                                       | 32 GB                                       |
| 저장 공간 | 8GB HDD                                                    | 20GB SDD                                    |

### **Mac**

|       | 최소 요구 사항                     | 권장 설정                      |
| ----- | ---------------------------- | -------------------------- |
| 운영체제  | macOS 11 Big Sur             | macOS 12 Monterey          |
| CPU   | Apple M1                     | Apple M1 Pro/M2            |
| GPU   | Apple M1 통합 GPU(Metal 지원 필요) | Apple M1 Pro/M2 Pro 통합 GPU |
| VRAM  | 6 GB                         | 12 GB                      |
| RAM   | 16GB                         | 32 GB                      |
| 저장 공간 | 8GB HDD                      | 20GB SDD                   |

### 그래픽 설정 조정

컴퓨터가 Decentraland의 최소 하드웨어 요구 사항을 모두 충족하지 못하더라도, 게임 내에서 그래픽 설정을 낮춰 경험을 최적화해 볼 수 있습니다. 최상의 성능을 위한 설정은 다음과 같습니다:

![](/docs/_remote/154c3847b6bd7b0396f9089c.webp)

## 사운드 설정

각각의 사운드 음량을 조정하여 Decentraland 경험을 사용자 지정하세요:

* **마스터:** 모든 사운드에 영향을 줌
* **UI 효과음:** Decentraland에 로그인할 때 재생되는 음악에 영향을 줌
* **게임 내 음악 및 효과음:** 음악, 스트림 오디오, 그리고 씬에서 생성되는 기타 모든 효과음에 영향을 줌
* **아바타 및 이모트 효과음:** 아바타와 효과음이 있는 이모트의 사운드에 영향을 줌

![](/docs/_remote/2b2767783d43b899be6c38c3.webp)

## 컨트롤 설정

여기에서 게임 내 마우스 감도를 조정할 수 있습니다.

![](/docs/_remote/821016ae97631107c7bb7d45.webp)

## 채팅 설정

기본 설정에서 모든 채팅의 알림 및 채팅 말풍선 환경설정과 DM 권한을 한 번에 수정할 수 있습니다. 자세한 내용은 [**채팅**](/docs/player/player-ko/in-world/friends-and-chatting/#chat-settings) 섹션을 참조하세요.

![](/docs/_remote/d045987d8716c9125d8f9712.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/in-world/settings-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
