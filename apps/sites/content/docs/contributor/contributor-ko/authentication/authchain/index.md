# 인증 체인

Decentraland 프로토콜의 많은 작업은 사용자 이더리움 계정의 서명을 통한 승인이 필요하거나 그로부터 이점을 얻습니다. 예를 들면:

1. 다음의 새 버전을 업로드합니다 [엔티티](/docs/contributor/contributor-ko/content/entities/) 사용자가 소유한 것(예: 프로필).
2. 제3자 서비스에 자신을 인증합니다.
3. 대리인이 대신 행동하도록 승인합니다.

이러한 작업을 수행하려면 사용자와 대리인은 자신의 의도를 설명하는 페이로드에 서명하고 다음을 생성해야 합니다. *인증 체인*.

## 소개

인증 체인은 일련의 검증 단계를 캡슐화하며, 각 단계는 이전 단계의 검증 성공 여부에 의존합니다. 제한된 작업이 실행되려면 검증자가 모든 단계를 유효하다고 판단해야 합니다.

모든 체인은 사용자 식별로 시작하여 요청된 작업을 나타내는 서명된 페이로드로 끝납니다. 따라서 가장 작은 체인은 다음을 나타내는 두 요소로 구성됩니다:

```md
1. 권한은 이더리움 계정 <user-address>입니다
2. 페이로드는 <payload>이며, <user-signature>에 의해 승인되었습니다
```

이 기본 체인은 서명의 공개 키가 해당 주소와 일치하는지 확인하여 평가할 수 있습니다. 이 경우 일반 서명과 동일합니다.

사용자가 대리인이 대신 행동하도록 승인하면 체인에 중간 단계가 나타납니다. 예를 들어, 단일 위임이 있는 체인은 다음을 나타냅니다:

```md
1. 권한은 이더리움 계정 <user-address>입니다
2. 대리인은 <delegate-address>이며, <date>까지 <user-signature>에 의해 승인되었습니다
3. 페이로드는 <payload>이며, <delegate-signature>에 의해 승인되었습니다
```

대리인이 자신의 대리인을 승인하면 체인이 더 길어집니다. 다시 말해, 승인은 전이적입니다.


> [!NOTE]
> 인증 체인을 HTTPS에 사용되는 TLS 인증서 체인에 비유할 수 있으며, 사용자가 루트 권한에 해당합니다.


이 단일 대리인 체인은 Decentraland에서 사용되는 가장 일반적인 승인 형태인데, 사용자가 World Explorer용 키를 승인하여 이더리움 계정으로 모든 개별 작업에 서명할 필요를 피하기 때문입니다.

### 체인 구성 <a href="#constructing" id="constructing"></a>

인증 체인의 각 단계에는 세 가지 정보가 포함됩니다. 즉, a `유형`, a `페이로드` 및 이에 대응하는 `서명`.

| 필드     | 값                                                   |
| ------ | --------------------------------------------------- |
| `유형`   | 유형의 이름([아래 참조](#types)).                            |
| `페이로드` | 유형에 따라 달라지는 문자열.                                    |
| `서명`   | 다음의 16진수로 인코딩된 Ethereum 서명: `페이로드`, 다음으로 시작하는 `0x`. |


> [!NOTE]
> 인증 체인의 가장 일반적인 직렬화 형식은 JSON 배열이므로, 아래 예제는 그 형식으로 제시됩니다. 자세한 내용은 [체인 전송](#transmitting) 을 참조하십시오.


### 첫 번째 단계: 식별

원래 권한자(즉, 사용자)를 식별하는 첫 번째 단계는 다음 조건을 충족해야 합니다:

1. 이 `유형` 은 [`SIGNER`](#signer)
2. 이 `페이로드` 은 인코딩된 이더리움 계정입니다.
3. 이 `서명` 은 비어 있어야 합니다.

예를 들면:

```json
// 모든 인증 체인의 첫 번째 단계:
{
  "type": "SIGNER",
  "payload": "0xdB055877e6c13b6A6B25aBcAA29B393777dD0a73",
  "signature": ""
}
```

두 번째 단계는 다음을 포함해야 합니다. `서명` 이 계정에서 그 목적을 위한 `페이로드`.

### 중간 단계: 위임

대리인이 사용자를 대신해 행동할 때, 각 대리인에 대해 체인 중간에 항목이 하나 추가됩니다. 이러한 단계의 조건은 다음과 같습니다:

1. 유형은 [`ECDSA_EPHEMERAL`](#ecdsa_ephemeral)
2. 이 `페이로드` 아래를 참조하십시오. 특별히 작성된 텍스트입니다.

이 `페이로드` 사람(지갑 UI를 사용할 때)과 프로그램(작성 및 검증할 때) 모두가 이를 다뤄야 하므로, 읽기 쉽고 파싱하기 쉽게 설계되었습니다. 대소문자를 구분하는 일반 텍스트 3줄만 포함합니다:

```md
<purpose>
임시 주소: <delegate-address>
만료: <date>
```

예를 들어, 다음은 World Explorer가 로그인할 때 사용자가 승인할 임시 대리인 키를 생성하면서 사용하는 일반적인 페이로드입니다:

```
Decentraland 로그인
임시 주소: 0xBB9aDF0183b1742196A4Aa55622D5838f4f483a7
만료: 2023-02-25T13:00:19.730Z
```

다음 사항에 유의하십시오:

1. 이 `임시 주소` 는 블록체인에서 자금이 들어 있는 실제 이더리움 계정일 필요가 없습니다. 이 필드는 임시 공개 키만 나타낼 수 있습니다.
2. 이 `만료` 는 다음 형식의 직렬화된 날짜-시간입니다: [ISO-8601](https://en.wikipedia.org/wiki/ISO_8601) 형식입니다. UTC일 필요는 없습니다.
3. 이 만료일 때문에 대리인 키는 사용자로부터 새 서명으로 주기적으로 갱신되어야 합니다.


> [!NOTE]
> 갱신할 때마다 새 키를 생성하는 것이 표준 관행이며(따라서 이름이 `ECDSA_EPHEMERAL`).


예시 위임 단계(명확성을 위해 값 축약):

```json
{
  "type": "ECDSA_EPHEMERAL",
  "payload": "Decentraland Login\nEphemeral address: 0xBBa7...\nExpiration: 2021-01-25T...",
  "signature": "0x1370a4120a7cb0d2f6e4a5..."
}
```

다음 단계는 또 다른 위임이든 최종 승인 단계이든, 이 대리인의 키로 생성된 서명을 포함합니다.

### 마지막 단계: 승인

다음 이후, `SIGNER` 이 지정되고 모든 `ECDSA_EPHEMERAL` 중간 서명의 체인을 검증하여 대리인 키가 모두 검증되면, 마지막 단계는 승인이 필요한 실제 작업입니다.

이 `페이로드` 는 다음에 따라 달라집니다 `유형` 단계의. 예를 들어, 사용자가 콘텐츠 서버에 새 프로필(또는 자신이 소유한 다른 엔티티)을 업로드하는 경우, 마지막 요소는 다음 형식을 가집니다:

```json
{
  // 유형은 `payload`가 엔티티 ID임을 나타냅니다(자세한 내용은 Content 섹션 참조):
  "type": "ECDSA_SIGNED_ENTITY",

  // 페이로드는 원시 ID 문자열입니다:
  "payload": "bafkreicfbg7ybpuoslkcf6x2vfnvzl5vwgqtb2pnheqiut2i4sgpblicqi",

  // 서명은 이전 단계의 계정(사용자 또는 대리인)에서 생성됩니다:
  "signature": "0x7e71dbbab..."
}
```

이 마지막 서명도 유효하면 작업을 진행할 수 있습니다.

### 체인 전송 <a href="#transmitting" id="transmitting"></a>

앞서 언급했듯이, 인증 체인의 가장 일반적인 직렬화 형식은 JSON 배열입니다. 이것이 권장되는 방법입니다.

하지만 프로토콜이 이를 강제하지는 않습니다. 개발자는 특정 사용 사례에 더 편리하다면 YAML, CSV 파일, 최적화된 바이너리 형식 또는 구분된 필드가 있는 단순 문자열과 같은 대체 직렬화 방식을 사용할 수 있습니다.

프로토콜 내부에서의 대체 직렬화 예시는 다음에서 찾을 수 있습니다. [`SignedFetch`](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/signed-fetch/) 모듈로, JSON 배열 대신 일련의 HTTP 헤더를 사용합니다.

### 만료일 선택

대리인 키의 유효 기간을 선택할 때는 절충점이 있습니다. 만료 기간이 짧을수록 보안은 높아지지만, 만료 기간이 길수록 사용자 경험이 향상됩니다(대리인을 사람의 상호작용으로 갱신해야 하는 빈도가 낮아지기 때문입니다).

유효 시간 창이 얼마여야 하는지에 대한 만능 전략은 없습니다. 참고로 Foundation의 World Explorer는 대리인 키에 대해 한 달의 승인을 요청합니다.


> [!NOTE]
> 임시 키는 절대 자금을 보유하거나 디지털 자산을 이전할 수 있는 능력을 가져서는 안 됩니다. 임시 키가 유출되어도 금전적 손실로 이어지지 않는 것이 최종 사용자에게 훨씬 더 안전합니다.


## 형식화

다음은 관련 프로세스에 대한 보다 형식적이고 정확한 정의입니다. 인증 체인을 올바르게 처리하려면 다음 지침을 따르십시오.

### 생성

사용자를 위한 인증 체인을 구성하는 클라이언트는 다음 단계를 따릅니다:

1. 식별 단계를 추가합니다:
   1. 다음을 설정합니다 `유형` 을(를) `SIGNER`.
   2. 다음을 설정합니다 `페이로드` 사용자의 이더리움 주소로.
   3. 다음을 설정합니다 `서명` 을(를) 빈 문자열로.
2. 위임 단계를 추가합니다:
   1. 기존 대리인 개인 키를 생성하거나 사용합니다(상호작용이 필요할 수 있음).
   2. 해당 공개 키에서 파생된 대리인 Ethereum 주소를 계산합니다.
   3. 다음을 설정합니다 `유형` 을(를) `ECDSA_EPHEMERAL`.
   4. 다음을 설정합니다 `만료` 을(를) 미래의 날짜로 설정합니다.
   5. 다음을 선택합니다 `목적` 을(를) 이 키에 대해.
   6. 다음을 설정합니다 `페이로드` 을(를) 다음 정확한 형식으로:

      ```md
      <purpose>
      임시 주소: <delegate-address>
      만료: <date>
      ```
   7. 다음을 설정합니다 `서명` 필드에 `페이로드` 이전 키(사용자 또는 대리인)의 서명을.
   8. 이후의 모든 대리인에 대해 반복합니다.
3. 작업 승인 단계를 추가합니다:
   1. 다음을 설정합니다 `유형` 을(를) 유효한 값으로([아래 참조](#types))
   2. 다음을 설정합니다 `페이로드` 을(를) 유형별 값(예: 엔티티 ID)으로.
   3. 다음을 설정합니다 `서명` 필드에 `페이로드` 이전 키(사용자 또는 대리인)의 서명을.
4. 인증 체인을 검증자에게 전송합니다.

### 검증

검증을 구현하는 콘텐츠 서버와 제3자 서비스는 다음 단계를 따릅니다:

1. 식별을 검증합니다:
   1. 다음을 검증합니다 `유형` 은 `SIGNER`.
   2. 다음을 검증합니다 `페이로드` 이 사용자의 이더리움 주소입니다.
   3. 다음을 검증합니다 `서명` 은(는) 빈 문자열입니다.
2. 대리인을 검증합니다:
   1. 다음을 검증합니다 `유형` 은 `ECDSA_EPHEMERAL`.
   2. 다음을 검증합니다 `페이로드` 이 형식인지 확인하고 필드를 추출합니다:

      ```md
      <purpose>
      임시 주소: <delegate-address>
      만료: <date>
      ```
   3. 다음을 검증합니다 `날짜` 가 아직 미래여야 합니다.
   4. 다음을 검증합니다 `목적` 가 귀하의 서비스에서 지원되어야 합니다.
   5. 다음을 검증합니다 `서명` 가 주어진 `페이로드` 및 이전 공개 키에 대해 유효해야 합니다.
   6. 이후의 모든 대리인에 대해 반복합니다.
3. 작업 승인을 검증합니다:
   1. 다음을 검증합니다 `유형` 이(가) 유효한 값이어야 합니다([아래 참조](#types)).
   2. 다음을 검증합니다 `페이로드` 이(가) 이 `유형`.
   3. 다음을 검증합니다 `서명` 가 주어진 `페이로드` 및 이전 공개 키에 대해 유효해야 합니다.
4. 인증 체인을 수락합니다.

### 표준 작업 유형 및 목적 <a href="#types" id="types"></a>

이 `유형` 및 `페이로드` 식별 및 위임에 대한 값은 표준이며 위에서 설명한 대로 검증해야 하지만, 클라이언트와 서비스는 임의의 작업 `유형`, 그리고 그 `페이로드` 구조와, 또한 다음을 설정할 수 있습니다 `목적` 대리인 키에 대해 자신들이 유효하다고 간주하는

프로토콜은 세 가지 표준 유형과 하나의 표준 목적을 정의합니다:

#### 유형 `SIGNER` <a href="#signer" id="signer"></a>

**다음이어야 합니다** 체인의 초기 `유형` 이어야 하며, 여기서 `페이로드` 는 사용자의 이더리움 주소이고 `서명` 은(는) 빈 문자열입니다.

#### 유형 `ECDSA_EPHEMERAL` <a href="#ecdsa_ephemeral" id="ecdsa_ephemeral"></a>

**다음이어야 합니다** 이어야 합니다 `유형` 체인의 중간 단계에 대한 `페이로드` 는 위에서 설명한 형식이어야 합니다.

#### 유형 `ECDSA_SIGNED_ENTITY` <a href="#ecdsa_signed_entity" id="ecdsa_signed_entity"></a>

마지막 `유형` 엔티티 배포를 승인하기 위한 체인의 항목이어야 하며, 여기서 `페이로드` 는 사용자가 소유한 엔티티의 ID입니다.

#### 목적 `Decentraland 로그인` <a href="#decentraland-login" id="decentraland-login"></a>

일반적인 `목적` World Explorer용이며, 월드에 로그인할 때와 대리인 키를 갱신할 때 모두 서명됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/authentication/authchain.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
