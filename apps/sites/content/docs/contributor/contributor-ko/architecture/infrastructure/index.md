# 인프라

이 페이지는 Decentraland를 구동하는 지원 인프라와 외부 서비스를 다룹니다.

## 외부 서비스(SaaS)

### 라이브킷

**목적**: 음성 및 비디오를 위한 고품질 미디어 서버

[라이브킷](https://livekit.io/) 다음을 제공합니다:

* WebRTC 미디어 라우팅
* 음성 채팅
* 화상 회의
* 화면 공유
* 공간 음향(개발 중)

**접근 제어**: [Comms Gatekeeper](https://github.com/decentraland/docs/tree/main/apis/apis/comms-gatekeeper/README.md) 시간 제한 토큰을 생성합니다

**사용 대상**:

* Archipelago Workers - 기본 음성
* Worlds - 월드별 음성 채널
* Events - 이벤트별 채널

### SendGrid

**목적**: 이메일 전송 서비스

알림 이메일을 전송합니다:

* 친구 요청
* 이벤트 알림
* 시스템 알림

**트리거 대상**: [알림 시스템](https://github.com/decentraland/docs/tree/main/apis/apis/notifications-workers/README.md)

***

## 블록체인 인덱서

여러 인덱싱 서비스가 빠른 블록체인 쿼리를 제공합니다:

### Squids

**목적**: 맞춤형 블록체인 인덱서

[Subsquid](https://www.sqd.ai/) 기반 인덱서:

* 맞춤형 데이터 모델
* 빠른 동기화 시간
* GraphQL API

**색인 대상**:

* 토지 구획(LAND NFT)
* 웨어러블 및 컬렉션
* 마켓플레이스 활동
* ENS 이름

### Satsuma

**목적**: 호스팅형 GraphQL 제공업체

관리형 서브그래프 호스팅:

* 고가용성
* 자동 확장
* 모니터링 포함

### The Graph

**목적**: 탈중앙화 인덱싱 프로토콜

[The Graph](https://thegraph.com/) 네트워크:

* 탈중앙화 인덱서
* GRT로 지불하는 쿼리 수수료
* 커뮤니티 운영

***

## 메시지 브로커

### NATS

**목적**: 발행/구독 메시지 라우팅

[NATS](https://nats.io/) 다음을 제공합니다:

* 실시간 메시지 전달
* 발행/구독 패턴
* 요청/응답 패턴
* 클러스터링 지원

**사용 대상**:

| 서비스         | 목적      |
| ----------- | ------- |
| Archipelago | 위치 업데이트 |
| 소셜 서비스      | 친구 상태   |
| 월드          | 월드 이벤트  |

***

## 미디어 처리

### 이미지 변환 서비스(Metamorph)

**목적**: 이미지 형식 변환 및 최적화

이미지를 다음 형식 간에 변환합니다:

* WebP 최적화
* 썸네일 생성
* 형식 변환

**GitHub**: [metamorph](https://github.com/decentraland/metamorph)

### 프로필 이미지 시스템

**목적**: 아바타 썸네일 생성

아바타 미리보기 이미지를 생성합니다:

* 얼굴 샷
* 전신 샷
* 여러 해상도

**구성 요소**:

* **프로필 이미지 소비자** - 아바타를 렌더링합니다
* **프로필 생성자** - 아바타 변경을 수신합니다

***

## 자산 처리 워커

### 자산 번들 변환기

**목적**: 플랫폼별 자산 최적화

배포된 자산을 모든 플랫폼용 최적화 번들로 변환합니다.

**번들을 다음에 등록합니다**: [자산 번들 레지스트리](https://github.com/decentraland/docs/tree/main/apis/apis/asset-bundle-registry/README.md)

### LOD 생성기

**목적**: 디테일 수준 변형

더 나은 성능을 위해 3D 모델의 저폴리곤 버전을 생성합니다.

***

## 보상 시스템

### 보상 API

**목적**: NFT 보상 분배

다음 행동에 대해 NFT 보상을 제공합니다:

* 온보딩 완료
* 이벤트 참여
* 추천

***

## 다음 단계

* 검토 [백엔드 서비스](/docs/contributor/contributor-ko/architecture/services/) 서비스 사양용
* 확인 [Catalyst 네트워크](/docs/contributor/contributor-ko/architecture/catalyst/) 콘텐츠 인프라용
* 참고 [아키텍처 개요](/docs/contributor/contributor-ko/architecture/architecture/) 전체 그림은


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/architecture/infrastructure.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
