> 크레딧 선물하기는 초대 전용 베타 프로그램입니다. 참여 스튜디오는 상점의 스튜디오 페이지에서 플레이어에게 크레딧을 선물할 수 있습니다.

# 크레딧 선물하기(베타)

크레딧 선물하기는 현재 초대 전용 베타 프로그램입니다. 참여 스튜디오는 플레이어에게 크레딧을 선물할 수 있습니다.

스튜디오가 참여하도록 초대받았다면, 다음에서 Shop의 스튜디오 페이지를 통해 크레딧을 선물할 수 있습니다. [decentraland.org/shop/studio](https://decentraland.org/shop/studio). 플레이어는 Shop의 모든 항목에 이 크레딧을 사용할 수 있습니다.

## 시작하기 전에

* **귀하의 계정은 스튜디오를 관리할 수 있어야 합니다.** Decentraland가 귀하의 스튜디오에서 선물을 보낼 수 있는 계정을 추가합니다. 로그인했는데 *이 계정은 스튜디오를 관리하지 않습니다*라는 메시지가 보이면, 해당 계정을 추가해 달라고 Decentraland 담당자에게 요청하세요.
* **스튜디오 자체 계정은 해당 크레딧을 받을 수 없습니다.** 테스트 선물에는 다른 계정을 사용하세요.
* **선물은 최종적입니다.** 선물한 크레딧은 만료되지 않으며 회수할 수도 없으니, 확인하기 전에 목록을 검토하세요.

## 스튜디오 페이지

다음에서 스튜디오 페이지에 로그인하세요. [decentraland.org/shop/studio](https://decentraland.org/shop/studio). 계정이 하나 이상의 스튜디오를 관리하는 경우, **스튜디오** 메뉴에서 선택하세요. 페이지에는 다음이 표시됩니다:

* **남은 예산**: 스튜디오가 아직 선물할 수 있는 크레딧입니다.
* **지금까지 선물한 양**: 스튜디오가 선물한 내역과 그 횟수입니다.
* **플레이어당 최대**: 한 명의 플레이어가 한 번에 받을 수 있는 최대 선물입니다.
* **선물**: 스튜디오가 보낸 모든 선물로, 플레이어, 크레딧, 사유, 날짜, 보낸 사람이 포함됩니다.

![남은 예산, 지금까지 선물한 크레딧, 플레이어당 최대치 및 선물 목록이 표시된 스튜디오 페이지](/docs/_remote/eca2a726b6b3c94c7710e0b5.webp)

페이지에 스튜디오가 일시 중지되었다고 표시되면, Decentraland가 재개할 때까지 선물을 보낼 수 없습니다.

## 크레딧 선물하기

1. 눌러 **크레딧 선물하기**.
2. 작성하세요 **사유**. 이는 자체 사유가 없는 모든 플레이어에게 사용됩니다: 한 줄에 최대 80자, 예: *이번 주 최고 플레이어*.
3. 플레이어를 추가하세요. 한 줄에 하나씩: 계정( `0x…` 주소)과 선물할 크레딧을 입력합니다. 각 행에 별도의 사유를 지정할 수 있습니다.
   * **플레이어 추가** 한 행을 추가합니다.
   * **목록 붙여넣기** 는 한 줄당 한 명의 플레이어를 입력합니다: 계정, 크레딧, 그리고 선택 사항인 사유. 스프레드시트에서 복사한 열도 사용할 수 있습니다.

     ```
     0x…, 100, 이번 주 최고 플레이어
     0x…, 50
     ```
   * 금액은 천 단위 구분 기호 없이 작성하세요(`1000`, 아니라 `1,000`).
   * 각 계정은 한 행에만 들어갑니다: 플레이어가 두 금액을 받으면 합산하세요.
4. 목록 아래의 요약(플레이어 수, 총 크레딧, 그리고 이후 남은 예산)을 확인한 다음, **검토**.

   ![사유와 두 명의 플레이어가 있으며, 각 플레이어마다 개별 크레딧과 사유가 있는 크레딧 선물 창](/docs/_remote/24ad20ec86951563169233a0.webp)
5. 검토 단계에서는 지난 30일 동안 귀하의 스튜디오에서 같은 선물(동일한 크레딧과 사유)을 받은 플레이어를 경고합니다. 다시 선물하려는 의도가 아니라면 돌아가서 그 플레이어를 제거하세요.
6. 총 크레딧을 입력하여 확인한 다음, **…명의 플레이어에게 선물**.

   ![총 크레딧을 입력한 상태의 확인 단계와 '2명의 플레이어에게 선물' 버튼](/docs/_remote/f677deb1676df2cd4f1c5912.webp)

선물은 하나씩 전송되며 각 행에 진행 상태가 표시됩니다. 창이 닫히거나 목록을 중단하면, **크레딧 선물하기** 다시 열고 **계속**: 이미 선물을 받은 플레이어는 중복으로 받지 않습니다.

## 플레이어가 받는 것

* 귀하의 스튜디오가 크레딧을 선물했다는 Decentraland 알림.
* Decentraland 계정에서 이메일 주소를 확인한 경우 이메일.
* Shop 활동 내 선물 내역과 잔액의 크레딧이 바로 사용 가능한 상태로 표시됩니다.

## 선물이 처리되지 않으면

| 메시지                  | 조치 방법                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------- |
| 이 계정은 귀하의 스튜디오에 속합니다 | 스튜디오 자체 계정은 해당 크레딧을 받을 수 없습니다. 행을 제거하세요.                                                 |
| 이 계정은 선물을 받을 수 없습니다  | 이 플레이어는 크레딧을 받을 수 없습니다. 행을 제거하세요.                                                        |
| 플레이어당 최대치를 초과함       | 금액을 다음 값으로 낮추세요: **플레이어당 최대** 스튜디오 페이지에 표시된                                              |
| 남은 예산이 충분하지 않습니다     | 더 적게 선물하거나, 예산에 대해 Decentraland 담당자와 상의하세요.                                              |
| 스튜디오가 일시 중지되었습니다     | Decentraland가 스튜디오를 재개할 때까지 기다린 다음, **계속**.                                              |
| 확인을 받지 못했습니다         | 확인하세요 **선물** 다시 보내기 전에 목록을 확인하세요: 선물이 이미 전달되었을 수 있습니다.  **계속** 절대 같은 사람에게 두 번 선물하지 않습니다. |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/rewards/gifting-credits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
