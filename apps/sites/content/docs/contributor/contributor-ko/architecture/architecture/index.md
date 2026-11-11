# 개요

디센트럴랜드는 블록체인 소유권, 분산형 콘텐츠 전달, 중앙화된 서비스를 결합한 하이브리드 아키텍처 위에 구축된 분산형 가상 세계 플랫폼으로, 향상된 사용자 경험을 제공합니다.

## 아키텍처 원칙

### 중요한 곳에서의 탈중앙화

* **소유권** - LAND, 착용 아이템, 이름은 이더리움의 NFT입니다
* **거버넌스** - DAO가 핵심 스마트 계약과 금고를 관리합니다
* **콘텐츠** - Catalyst 노드 전반에 분산됨
* **신원** - 지갑 기반 인증(비밀번호 없음)

### 성능을 위한 중앙화

* **실시간 서비스** - WebSocket 통신, 음성 채팅
* **디스커버리** - 검색, 이벤트, 추천 콘텐츠
* **소셜 기능** - 친구, 커뮤니티, 알림
* **최적화** - 자산 처리, 이미지 생성

## 시스템 계층

### 클라이언트

플레이어는 다양한 클라이언트를 통해 상호작용합니다:

* **Unity Explorer** - 기본 데스크톱 클라이언트(Windows, Mac, Linux)
* **Web Explorer** - 브라우저 기반 클라이언트
* **CLI** - 씬 생성 및 배포를 위한 개발자 도구
* **커뮤니티 클라이언트** (모바일, VR 등) - 다양한 렌더링 기술을 사용한 참조 클라이언트의 대체 구현입니다.

모든 클라이언트는 암호화 지갑으로 인증하며 동일한 프로토콜을 따릅니다.

### 디스커버리(Realm Provider)

클라이언트에 realm 설명을 제공하는 단일 진입점:

* 사용 가능한 Catalyst 노드(위치, 용량, 상태)
* 백엔드 서비스 엔드포인트

지리와 부하를 기반으로 지능적인 서버 선택을 가능하게 합니다.

### 분산형 콘텐츠(Catalyst 네트워크)

다음을 저장하고 제공하는 무허가형 Catalyst 노드 네트워크:

* 씬 정의 및 자산
* 아바타 프로필 및 착용 아이템

각 노드는 콘텐츠를 독립적으로 검증하고 저장합니다. 노드들은 배포를 동기화하지만 자율적으로 운영됩니다. 참조 [Catalyst 네트워크](/docs/contributor/contributor-ko/architecture/catalyst/) 자세한 내용은

### 중앙화된 기능 서버

백엔드 마이크로서비스는 향상된 기능을 제공합니다:

* **서비스 디스커버리** - Realm 선택 및 상태 모니터링
* **통신** - WebSocket 클러스터링, 음성/비디오 접근 제어
* **콘텐츠 기능** - 월드, 이벤트, 장소, 아틀라스 데이터
* **소셜 기능** - 친구, 커뮤니티, 알림
* **플랫폼 기능** - 업적, 보상, 가상 크레딧
* **자산 처리** - 최적화, 썸네일, 번역

참고 [백엔드 서비스](/docs/contributor/contributor-ko/architecture/services/) 포괄적인 서비스 문서를 위해.

### 인프라

지원 시스템 및 타사 서비스:

* **메시지 브로커** - 서비스 간 통신을 위한 NATS
* **미디어 서버** - 음성 및 비디오를 위한 LiveKit
* **블록체인 인덱서** - NFT 데이터에 대한 빠른 쿼리
* **이메일 서비스** - 알림을 위한 SendGrid

참고 [인프라](/docs/contributor/contributor-ko/architecture/infrastructure/) 자세한 내용은

### 블록체인

이더리움과 폴리곤의 스마트 계약이 다음을 관리합니다:

* **소유권** - LAND, 착용 아이템, 이름, 에스테이트
* **마켓플레이스** - 거래와 로열티
* **DAO** - 거버넌스와 금고
* **통화** - MANA 토큰

## 보안 모델

### 신원 및 인증

* **지갑 서명** - 비밀번호 없음, 이더리움 서명을 통한 인증
* **인증 서버** - 서명 검증 후 JWT 토큰을 발급합니다
* **서명된 가져오기** - API가 지갑 서명을 사용하여 요청을 인증합니다

### 콘텐츠 검증

* **서명 검증** - 모든 배포는 지갑으로 서명됩니다
* **포인터 소유권** - LAND 소유자만 좌표에 배포할 수 있습니다
* **크기 제한** - 콘텐츠 크기는 파셀 수에 의해 제한됩니다

### 접근 제어

* **시간 제한 토큰** - JWT 및 LiveKit 토큰은 만료됩니다
* **속도 제한** - IP/사용자별 API 속도 제한
* **DDoS 보호** - 분산형 Catalyst 네트워크

## 확장성 접근 방식

### 수평 확장

* **Catalyst 노드** - 콘텐츠가 증가함에 따라 노드를 추가합니다
* **백엔드 서비스** - 무상태 마이크로서비스는 독립적으로 확장됩니다
* **섬** - 통신이 근접 기반 그룹으로 분할됩니다

### 최적화

* **자산 번들** - 플랫폼 최적화 자산(Unity 번들)
* **CDN** - 정적 자산은 CDN을 통해 제공됩니다
* **캐싱** - 모든 계층에서 적극적인 캐싱
* **인덱서** - 블록체인 데이터는 고속 데이터베이스에 캐시됩니다

## 다음 단계

특정 아키텍처 영역을 더 깊이 살펴보세요:

* [**백엔드 서비스**](/docs/contributor/contributor-ko/architecture/services/) - 상세한 서비스 사양 및 통합
* [**Catalyst 네트워크**](/docs/contributor/contributor-ko/architecture/catalyst/) - 콘텐츠 전달 및 배포
* [**인프라**](/docs/contributor/contributor-ko/architecture/infrastructure/) - 지원 시스템 및 타사 서비스

## 추가 자료

* [API 참조](https://github.com/decentraland/docs/tree/main/apis/README.md) - 완전한 API 문서
* [기여자 가이드](/docs/contributor/contributor-ko/contributor-guides/contributor-guides/) - 개발 표준
* [GitHub 조직](https://github.com/decentraland) - 소스 코드 저장소


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/architecture/architecture.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
