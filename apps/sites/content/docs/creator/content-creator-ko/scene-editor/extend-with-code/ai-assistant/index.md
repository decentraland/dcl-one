> Creator Hub 안에서 AI 어시스턴트와 채팅하며 씬을 구축하고 변경하세요.

# AI 어시스턴트

Creator Hub는 열려 있는 씬을 보고 대신 편집해 주는 AI 도우미를 실행할 수 있습니다. 아이템 배치, 동작 연결, 스크립트 작성, 미리보기 실행 및 결과 확인을 시켜 보세요.


> [!WARNING]
> **📔 참고**: 도우미는 설치하고 로그인한 코딩 CLI를 실행하며, 기기 전체에 접근할 수 있어 터미널에서와 같은 범위의 권한을 가집니다. 신뢰할 수 있는 작업만 맡길 때만 활성화하세요.


## 설정이 있는 위치

도우미는 기본적으로 켜져 있습니다. 설정은 별도의 탭에 있습니다:

1. 왼쪽 상단의 Creator Hub 로고를 클릭하고 다음을 선택하세요 **설정**.
2. 다음으로 이동하세요 **AI** 탭에 표시됩니다.

해당 탭에는 다음이 있습니다 **AI 도우미 활성화** 스위치, **대신 이 API 키로 청구** 옵션, **연결** 코딩 CLI 목록과 **AI 도우미 MCP 서버 공개** 설정이 있습니다.

도우미가 켜져 있으면, **AI 도우미** 버튼(반짝이 아이콘)이 편집기 상단 바의 왼쪽, **코드** 버튼 왼쪽에 나타납니다. 클릭하면 씬 옆에 채팅 패널이 열립니다.

![왼쪽에 씬 이름이 있고 오른쪽에 반짝이 AI 도우미 아이콘, 코드, 미리보기, 게시 버튼이 있는 편집기 상단 바.](/docs/_remote/97b2496489d20fd6596ad9b5.webp)

![처음 열었을 때의 AI 도우미 패널: 맨 위에 새 채팅 드롭다운과 Claude라고 표시된 CLI 드롭다운, 환영 메시지, AI 사용 정보 카드, 선택된 엔티티 이름이 있는 선택 칩, 그리고 하단의 도우미에게 묻기 입력란.](/docs/_remote/dceaca65cd87dab45134934a.webp)

## 코딩 CLI 연결

도우미는 자체 API 키를 사용하지 않습니다. 설치하고 로그인한 코딩 CLI를 실행하므로, 작업 요금은 해당 도구의 구독에 청구됩니다.

네 가지가 지원됩니다: Claude, Codex, Cursor, Gemini. 각 도구는 해당 벤더의 설치 프로그램을 통해 터미널에서 설치합니다.

### CLI 설치

운영 체제에 맞는 명령을 실행하세요:

| CLI    | macOS 및 Linux                                           | Windows                                                                                      |
| ------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Claude | `curl -fsSL https://claude.ai/install.sh \| bash`       | `winget install Anthropic.ClaudeCode`                                                        |
| Codex  | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 \| iex"`   |
| Cursor | `curl https://cursor.com/install -fsS \| bash`          | `powershell -ExecutionPolicy ByPass -c "irm 'https://cursor.com/install?win32=true' \| iex"` |
| Gemini | `npm install -g @google/gemini-cli`                     | `npm install -g @google/gemini-cli`                                                          |

이것들을 직접 복사할 필요는 없습니다. Creator Hub는 두 곳에서 운영 체제에 맞는 정확한 명령과 해당 벤더의 설치 가이드 링크를 보여줍니다:

* CLI가 연결되지 않은 상태에서 패널을 처음 열 때 나타나는 채팅 패널의 설정 카드.
* 설정의 **AI** 탭에서 **연결**, 아래의 **터미널에서** 각 CLI 아래 섹션. 거기 있는 각 명령에는 **복사** 버튼이 있습니다.

벤더 가이드:

* [Claude Code 설치 가이드](https://code.claude.com/docs/en/terminal-guide)
* [Codex CLI 설치 가이드](https://developers.openai.com/codex/cli)
* [Cursor CLI 설치 가이드](https://cursor.com/docs/cli/installation)
* [Gemini CLI 설치 가이드](https://www.geminicli.com/docs/get-started/installation)

설치한 후 **다시 확인** 을 채팅 패널의 설정 카드에서 클릭하면 앱을 다시 시작할 필요 없이 Creator Hub가 새 CLI를 인식합니다. 해당 **AI** 탭은 열 때마다 자동으로 다시 확인합니다.


> [!WARNING]
> **📔 참고**: Claude Desktop은 Claude Code와 같은 것이 아닙니다. 도우미에는 Anthropic의 터미널 도구인 Claude Code가 필요하며, 데스크톱 앱에 로그인한다고 해서 그것에 로그인되는 것은 아닙니다. Creator Hub가 Claude Desktop은 찾았지만 Claude Code는 못 찾으면, 그 사실을 알려 주고 설치 명령을 보여 줍니다.
> 
> Gemini CLI에는 유료 Gemini API 키 또는 Gemini Code Assist Standard나 Enterprise 라이선스가 필요합니다.


### 로그인

| CLI    | 로그인                         |
| ------ | --------------------------- |
| Claude | `claude auth login`         |
| Codex  | `codex login`               |
| Cursor | `cursor-agent login`        |
| Gemini | 실행 `gemini` 를 실행하고 안내를 따르세요 |

터미널에서 로그인 명령을 실행하세요. 브라우저에서 완료됩니다.

Codex에는 바로 가기가 있습니다. 아직 설치되지 않았다면 Creator Hub가 설치하고 대신 로그인할 수 있습니다. 클릭하세요 **Codex로 로그인** 를 채팅 패널의 설정 카드에서, 또는 **로그인** 옆의 Codex **연결** 에서 **AI** 탭에서. Creator Hub가 로그인 절차를 실행하고 브라우저를 열어 주며, 그러면 같은 위치에 **로그아웃** 링크가 나타납니다. 위 명령으로 직접 Codex를 설치했다면 버튼은 없습니다. 다른 도구처럼 터미널에서 로그인하세요.

녹색 점과 **연결됨** 이 CLI 옆에 있으면 사용할 준비가 되었다는 뜻입니다.


> [!NOTE]
> **💡 팁**: 둘 이상의 CLI에 로그인되어 있으면 채팅 패널의 드롭다운에서 전환할 수 있습니다.


### 에이전트가 다시 로그인하라고 요청할 때

저장된 로그인은 CLI가 계속 연결된 것처럼 보여도 만료될 수 있습니다. CLI가 메시지에 "로그인하지 않음"이라고 응답하면 Creator Hub는 원시 명령줄을 채팅에 그대로 전달하는 대신 **로그인이 필요함** 상태와 호박색 점을 표시합니다.

채팅 패널과 **터미널에서** 섹션은 **AI** 탭에서 실행할 두 명령을 한 번에 하나씩, 브라우저에서 완료되는 순서로 보여 줍니다:

| CLI    | 로그아웃한 다음 다시 로그인                                      |
| ------ | ---------------------------------------------------- |
| Claude | `claude auth logout`, 다음으로 `claude auth login`       |
| Codex  | `codex logout`, 다음으로 `codex login`                   |
| Cursor | `cursor-agent logout`, 다음으로 `cursor-agent login`     |
| Gemini | 실행 `gemini`, 다음으로 `/auth logout`, 다음으로 `/auth login` |

Creator Hub가 Codex를 설치했다면 대신 **로그아웃** 을 사용하고, 그다음 **로그인** 에서 **AI** 탭을 사용하세요.

로그인한 뒤 메시지를 다시 보내세요. 에이전트는 내내 활성화된 상태로 유지되므로 처음부터 다시 설정할 필요가 없습니다.

### 대신 API 키로 청구

CLI 구독 대신 토큰당 요금을 내고 싶다면 설정의 **대신 이 API 키로 청구** 에서 **AI** 탭에서 `합니다. 해당 공급자의 API 키를 환경 변수에서 읽습니다:`, `OPENAI_API_KEY`, `CURSOR_API_KEY` 또는 `GEMINI_API_KEY`. 기본값은 꺼짐입니다.

이 방법은 완료할 수 없는 로그인 문제를 우회하는 데도 사용할 수 있습니다. CLI가 **로그인이 필요함** 상태일 때 Creator Hub는 **또는 API 키 사용** 힌트를 해당 변수 이름과 함께 표시합니다.

## 할 수 있는 일

도우미는 열려 있는 씬에서 작업합니다. 할 수 있는 일:

* **씬 편집**: 엔티티 생성 및 삭제, 컴포넌트 변경, [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) , 첨부 [스크립트](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/) , 그리고 읽기 및 쓰기 `scene.json`.
* **코드 작성** 당신 씬의 파일에서.
* **미리보기 실행** 및 조작: 스크린샷 찍기, 둘러보기, 클릭하기, 로그 읽기, 성능 확인.
* **선택 항목 읽기** , 그래서 엔티티를 선택한 뒤 "이것을 더 크게 만들어"라고 말할 수 있습니다.

변경 사항은 발생하는 즉시 저장되어 일반 실행 취소 기록에 들어갑니다. 전체 대화를 한 번에 되돌리려면 도우미의 응답에서 **AI 변경 사항 실행 취소** 를 사용하세요.

![엔티티 이름 바꾸기와 Mesh Renderer 추가 요청 후 편집기 오른쪽에 도킹된 AI 도우미 패널: 답변에는 실행한 도구가 나열되고, 결과가 설명되며, 아래에 AI 변경 사항 실행 취소 버튼이 표시됩니다.](/docs/_remote/6a907fb6c14f96a55c037af0.webp)

도우미가 작업하는 동안 변경 사항이 엔티티 트리와 캔버스에 실시간으로 나타납니다.

![오른쪽에 AI 도우미 패널이 도킹된 전체 씬 편집기, 엔티티 트리에 Stage 엔티티 이름 변경이 반영되어 있고, 도우미 응답 끝에 AI 변경 사항 실행 취소가 있는 모습.](/docs/_remote/1a8e6565c0eaf9d6cba56d1b.webp)

## 채팅 사용하기

* **새 채팅** 새 대화를 시작합니다.
* **채팅 기록** 이전에 한 대화와 실행된 지 얼마나 되었는지를 보여 줍니다. 대화는 프로젝트별로 저장되므로 씬을 다시 열면 채팅도 다시 나타납니다.
* **별도 창에서 열기** 도우미를 별도 창으로 띄웁니다. 두 번째 모니터에서 유용합니다. **편집기에 다시 도킹** 다시 돌려놓습니다.
* 패널은 더 넓게 또는 더 좁게 드래그할 수 있습니다.

하나의 **AI 사용 정보** 카드는 도우미가 연결된 계정의 토큰을 사용한다는 점을 알려 줍니다. 해당 계정의 사용량을 주시하세요.

도우미는 한 번에 하나의 메시지만 처리합니다. 답변이 진행되는 동안 **생각 중…** 표시가 긴 도구 호출 중에도 턴이 끝날 때까지 계속 표시됩니다. 두 번째 메시지를 보내도 첫 번째 메시지가 취소되지는 않습니다. 답변을 기다리거나 **중지** 를 눌러 직접 턴을 끝내세요.

어떤 답변은 계속하기 전에 질문을 던집니다. 너무 오래 답하지 않으면 "이 질문은 더 이상 활성 상태가 아닙니다."로 닫히며 다시 요청할 수 있습니다.

### 프롬프트에 파일 첨부

때로는 설명보다 도우미에게 무언가를 보여 주는 편이 더 쉽습니다. 어떤 메시지든 세 가지 방법으로 파일을 첨부할 수 있습니다:

* 메시지 상자 왼쪽의 클립 아이콘을 클릭하고 **파일 첨부**라고 표시된 항목을 선택해 하나 이상의 파일을 고르세요.
* 파일 탐색기에서 파일을 메시지 상자로 드래그하세요. **파일을 놓아 첨부** 힌트가 드래그하는 동안 나타납니다.
* 클립보드에서 이미지를 바로 붙여넣으세요. 예를 들어 방금 찍은 스크린샷처럼요.

첨부된 각 파일은 메시지 상자 위에 칩으로 표시되며 종류를 나타내는 아이콘이 있습니다: 이미지, 3D 모델(`.glb`, `.gltf`), 오디오(`.mp3`, `.wav`, `.ogg`, `.m4a`, `.flac`), 또는 그 밖의 일반 파일. 보내기 전에 칩의 **제거** 십자 아이콘을 클릭해 빼낼 수 있습니다. 한 메시지에 최대 8개 파일을 첨부할 수 있으며, 텍스트 없이 첨부 파일만 보낼 수도 있습니다.

드래그하거나 디스크에서 선택한 파일은 실제 경로 그대로 도우미에 전달되므로 아무것도 복사되지 않고 크기 제한도 없습니다. 도우미는 이미지를 직접 볼 수 있고, 씬 코드를 작성할 때 경로를 통해 모델이나 오디오 파일을 참조할 수 있습니다. 붙여넣은 이미지는 디스크에 파일이 없으므로 Creator Hub가 먼저 임시 파일로 저장합니다. 그런 파일은 개당 8MB로 제한됩니다.

Claude CLI가 너무 오래된 경우 패널에 경고가 표시됩니다. 터미널에서 `claude update` 를 실행해 해결하세요.

### 프롬프트에서 에셋을 가리키기

파일을 첨부하면 그 내용이 전송됩니다. 에셋을 언급하는 것은 더 가볍습니다. 프로젝트의 어떤 에셋을 뜻하는지만 도우미에 알려 주므로, 경로를 일일이 입력하는 대신 "이것을 사용해"라고 말할 수 있습니다.

도우미를 연 상태에서 **로컬 에셋** 탭의 임의의 에셋을 더블클릭하세요. 메시지 상자 위에 칩으로 나타나며 다음처럼 표시됩니다 **에셋:&#x20;*****파일명***, 또는 **내장 애니메이션:&#x20;*****이름*** 내장 아바타 애니메이션 중 하나의 이름입니다.

* 원하는 만큼 많은 에셋을 언급할 수 있습니다. 각 항목은 고유한 칩과 고유한 **제거** 십자
* 를 갖습니다. **언급은** 선택
* 칩 옆에 배치되므로 씬의 엔티티와 에셋을 동시에 가리킬 수 있습니다: "이 애니메이션을 저 NPC에 넣어".


> [!NOTE]
> **💡 팁**: 씬 루트를 선택한 것은 아무것도 선택하지 않은 것으로 간주되므로 도우미에 절대 전달되지 않습니다. 도우미가 작업하게 하려면 실제 엔티티를 선택하세요.


## Decentraland 스킬이 대신 설치됩니다

도우미가 실행되면 Creator Hub가 공식 [Decentraland SDK 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 을 다운로드하여 프로젝트에 연결하므로, 도우미는 추측하지 않고 검증된 SDK7 패턴을 따릅니다.

프로젝트 폴더 안의 `.claude/skills` 및 `.agents/skills` 에 연결되며, 둘 다 `.gitignore` 에 추가되므로 저장소에 들어가지 않습니다.

## 도우미가 씬 폴더에 남기는 파일

도우미는 로컬 MCP 서버를 통해 열려 있는 씬을 읽고 편집할 수 있습니다. 각 CLI에는 그 서버를 알리는 방식이 다르므로, 사용하는 도구에 따라 프로젝트에 새 파일이 생길 수 있습니다:

| CLI    | 씬 폴더에 표시되는 항목                                           |
| ------ | ------------------------------------------------------- |
| Cursor | `.cursor/mcp.json`, 그리고 `.cursor/rules/creator-hub.mdc` |
| Gemini | `.gemini/settings.json`                                 |
| Claude | 아무것도 없습니다. 구성은 프로젝트 밖의 임시 파일에 들어갑니다.                    |
| Codex  | 아무것도 없습니다. 구성은 명령줄로 전달됩니다.                              |

이미 해당 구성 파일 중 하나가 있다면 Creator Hub는 자신의 서버를 덮어쓰지 않고 그 안에 항목을 병합합니다.


> [!WARNING]
> **📔 참고**: `.cursor/mcp.json` 는 액세스 토큰 자체를 포함하는 유일한 파일입니다. Cursor의 구성 형식에는 환경 변수에서 이를 읽어 오는 방법이 없기 때문입니다. Creator Hub가 이를 당신의 `.gitignore` 에 추가한 뒤 기록하므로, 커밋하지 말고 그 줄을 제거하지 마세요. Gemini 파일은 토큰을 `$CREATOR_HUB_MCP_TOKEN` 환경 변수로 참조하므로 비밀 정보가 없고 무시되지도 않습니다.


## 대신 자신이 쓰는 AI 도구 사용

이미 Claude Code, Cursor, Codex 또는 MCP를 지원하는 다른 도구를 사용 중이라면, 내장 패널 대신 열린 씬을 가리키게 할 수 있습니다. MCP(Model Context Protocol)는 AI 도구가 다른 프로그램을 호출하기 위한 표준 방식입니다.

1. Creator Hub 로고에서 메인 메뉴를 열고 다음을 선택하세요 **설정**, 그리고 **AI** 탭에서 확장 **AI 도우미 MCP 서버 공개**.
2. 클릭 **구성 복사**.
3. 하고 도구의 MCP 구성에 붙여넣으세요.

복사된 구성은 실제 포트와 토큰이 채워진 다음과 같습니다:

```json
{
  "mcpServers": {
    "creator-hub": {
      "type": "http",
      "url": "http://127.0.0.1:<PORT>/mcp",
      "headers": { "Authorization": "Bearer <TOKEN>" }
    }
  }
}
```

Claude Code의 경우 씬 폴더의 터미널에서 대신 추가할 수 있습니다:

```bash
claude mcp add --transport http --scope local creator-hub http://127.0.0.1:<PORT>/mcp \\
  --header "Authorization: Bearer <TOKEN>"
```


> [!WARNING]
> **📔 참고**: 포트와 토큰은 Creator Hub가 시작될 때마다 새로 생성됩니다. 도구에서 갑자기 `401` 오류가 나면 구성을 다시 복사하세요. 서버는 자신의 기기에서만 수신 대기하며, 항상 열려 있는 씬에만 작동합니다.


## 관련 페이지

* [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/): SDK 스킬과 효과적인 프롬프트 작성에 대한 전체 가이드.
* [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/): 코드 편집기에서 씬을 엽니다.
* [스크립트 컴포넌트 사용](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/): 파일에서 엔티티에 동작을 연결합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
