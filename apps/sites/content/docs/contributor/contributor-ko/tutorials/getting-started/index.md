# 시작하기

Decentraland Catalyst 노드가 어떻게 동작하는지 궁금했던 적이 있나요? 직접 노드를 운영해 보고 싶었지만 어디서 시작해야 할지 막막했던 적이 있나요? Decentraland용 콘텐츠를 만들고 있는데 프로덕션 서버에서 전체 개발 및 테스트 사이클을 수행하기가 어렵다고 느끼고 있나요?

그렇다면 더 이상 걱정하지 마세요. 이 튜토리얼은 계획을 세우고, 무엇을 결정해야 하는지 파악한 다음, 서버를 시작하는 과정을 단계별로 안내해 드립니다. 또한 계획대로 되지 않을 경우를 대비해, 끝부분에는 가장 흔한 몇 가지 문제와 이를 해결하는 방법을 담은 문제 해결 섹션도 있습니다.

준비되셨나요? 작업에 바로 들어갑시다.

### 하드웨어 요구 사항

Decentraland Foundation 서버는 Amazon AWS의 `t2.xlarge` 인스턴스에 배포됩니다. 따라서 해당 서버들은 공개 사용을 염두에 두고 있지만, 실제로는 사용 목적에 따라 훨씬 작은 사양의 자체 서버로도 충분할 수 있습니다.

AWS EC2 `t2.xlarge` 에는 다음 하드웨어가 있습니다:

* 4개의 vCPU.
* 16GB RAM

그리고 파일 저장소 + DB 저장소 용도로는 2TB 용량의 SSD / HDD면 충분하고도 남습니다. 이 글을 쓰는 시점에 Foundation 서버는 모든 저장소를 합쳐 1TB를 조금 넘게 사용하고 있습니다.

### 소프트웨어 사전 요구 사항

catalyst-owner를 실행하려면 다음이 필요합니다:

* Linux / MacOS 운영체제
* Docker / docker-compose
* Git
* LiveKit 클러스터

### 시작하기 전에 고려할 몇 가지 사항

* 노드를 인터넷에 공개적으로 노출하고 싶으신가요? 그렇다면 인터넷에 노출된 인스턴스와 연결할 수 있는 공개 URL이 필요합니다.
* 이 노드를 씬, 웨어러블 등의 개발용으로 사용할 예정인가요? 그렇다면 실제로는 수백 명의 사용자가 아니라 한두 명 정도만 접속할 가능성이 높으므로 하드웨어 사양은 훨씬 낮아도 됩니다.
* 모든 엔티티 유형을 동기화하고 싶으신가요? 이 서버를 씬 개발에 사용한다면 프로필 동기화는 건너뛰어도 될 것입니다. 프로필은 많은 시간과 대역폭, 그리고 무엇보다 디스크 공간을 차지하지만, 당신의 목적에는 큰 도움이 되지 않습니다.

### 라이브킷

플레이어 간의 통신을 조정하기 위해서는 Catalyst의 Comms 서비스가 외부 LiveKit 클러스터를 필요로 합니다. 이를 위해 두 가지 옵션이 있습니다: LiveKit Cloud 계정을 사용하거나 직접 LiveKit 클러스터를 운영하는 것입니다.

설정하는 것을 권장합니다 [LiveKit Cloud 계정](https://cloud.livekit.io/) 이 계정은 100명의 사용자를 관리할 수 있는 무료 티어가 제공되거나, 트래픽에 따라 요금을 지불하면 됩니다. 이 방식은 추가 인프라를 프로비저닝할 필요가 없고 서비스가 확장도 관리해 주므로 더 쉽습니다. 그렇지 않다면 LiveKit 클러스터를 프로비저닝해야 합니다. LiveKit의 문서는 매우 잘 정리되어 있습니다:

* [배포](https://docs.livekit.io/oss/deployment/)
* [분산 설정](https://docs.livekit.io/oss/deployment/distributed/)

*LiveKit 전송 기반 통신을 지원하는 데 필요한 기술적 세부 사항을 더 깊이 살펴보고 싶다면, 다음을 확인해 보세요.* [*ADR-70 새로운 통신 아키텍처*](https://adr.decentraland.org/adr/ADR-70)*.*

### 단계별 가이드

가장 먼저 할 일은 GitHub에서 [catalyst-owner](https://github.com/decentraland/catalyst-owner) 저장소를 클론하는 것입니다

```bash
> git clone https://github.com/decentraland/catalyst-owner.git
catalyst-owner'를 복제하는 중...
remote: 객체 나열 중: 2392개, 완료.
remote: 객체 계산 중: 100% (602/602), 완료.
remote: 객체 압축 중: 100% (168/168), 완료.
remote: 총 2392개(차이 506), 재사용 481개(차이 430), 팩 재사용 1790개
객체 받는 중: 100% (2392/2392), 926.05 KiB | 1.09 MiB/s, 완료.
델타 해석 중: 100% (1257/1257), 완료.
> cd catalyst-owner
```

그게 끝나면, 노드의 환경 변수를 설정하고 필요에 따라 변경할 차례입니다.

```bash
# /catalyst-owner 폴더에서
> cp .env.example .env
> cp .env-advanced.example .env-advanced
```

이제 선호하는 텍스트 편집기를 사용해 이 파일들에 필요한 변경을 가하세요. 예를 들어, `EMAIL` 환경 변수를 유효한 이메일로 설정해야 Certbot 인증서 만료에 대한 업데이트를 받을 수 있습니다.

또한 다음 값도 설정해야 할 수 있습니다. `CATALYST_URL` 특히 서버를 인터넷에 공개적으로 노출할 예정이라면 필요할 수 있습니다. 로컬 머신에서 사용할 경우 기본값인 `http://localhost` 이면 충분합니다.

**Comms 서비스**

LiveKit 클러스터를 사용할 수 있게 되면 Comms 서비스가 동작하도록 특정 LiveKit 변수를 설정해야 합니다. 예:

```
LIVEKIT_HOST=wss://livekit-1.mydomain.org
LIVEKIT_API_KEY=API-JjHuvM
LIVEKIT_API_SECRET=J7YSHmNzkNCEfT2
ROOM_PREFIX=my-prefix
```

이 `ROOM_PREFIX` 변수는 선택 사항이며 Catalyst가 생성한 LiveKit 룸을 식별하도록 설정할 수 있습니다. 이는 둘 이상의 Catalyst가 같은 LiveKit 클러스터를 사용할 때 유용합니다.

**콘텐츠 서버**

다음과 같은 변수 `CONTENT_SERVER_STORAGE` 가 있으며, 콘텐츠 서버가 저장소로 사용할 로컬 폴더를 정의합니다. 기본값은 `CONTENT_SERVER_STORAGE=./storage`입니다. 파일을 다른 위치에 저장하도록 이 값을 변경할 수 있으며, 적어도 참조된 폴더가 존재하는지 확인해야 합니다. 다음과 같이 만들어야 할 수 있습니다:

```bash
# /catalyst-owner 폴더에서 또는 CONTENT_SERVER_STORAGE가 가리키는 곳 어디에서든
> mkdir storage
```

또 다른 흥미로운 변수는 `SYNC_IGNORED_ENTITY_TYPES` 이며, 동기화 과정에서 특정 엔티티 유형을 무시할 수 있게 해줍니다. Catalyst 서버를 개발용으로 사용할 예정이라면, 모든 콘텐츠 유형을 동기화할 필요는 없을 수도 있습니다. 다음 env var를 설정할 수 있습니다. `SYNC_IGNORED_ENTITY_TYPES="profile,store"` 그래서 씬과 웨어러블만 DAO 서버에서 가져오게 됩니다. 이렇게 하면 서버의 시간, 대역폭, 디스크 저장 공간을 많이 절약할 수 있습니다.

> 💡 지원되는 환경 변수의 더 완전한 목록은 아래의 [환경 변수](#environment-variables) 섹션을 참고하세요.

#### Catalyst 노드 시작하기

모든 환경 변수를 설정했다면 이제 노드를 시작할 차례입니다. 루트 폴더에서 `init.sh` 스크립트를 실행하면 됩니다.

```bash
# /catalyst-owner 폴더에서

> ./init.sh
## 환경 변수 로드 중... [ OK ]
[ OK ]
## 이메일이 구성되었는지 확인 중... [ OK ]
## 저장소가 구성되었는지 확인 중... [ OK ]
## catalyst URL이 구성되었는지 확인 중... [ OK ]
 경고: Catalyst의 Content 및 Catalyst의 Lambdas 노드 최신 이미지를 실행하고 있지 않습니다.
 경고: Catalyst의 Archipelago 노드 최신 이미지를 실행하고 있지 않습니다.
 경고: Catalyst의 Explorer BFF 노드 최신 이미지를 실행하고 있지 않습니다.
 - DOCKER_TAG:               45a5154f11b53d55aadfdf7f958e2fce6a964824
 - LIGHTHOUSE_DOCKER_TAG:    latest
 - CATALYST_URL:             http://localhost
 - CONTENT_SERVER_STORAGE:   ./storage
 - EMAIL:                    a@a.com
 - ETH_NETWORK:              mainnet
 - REGENERATE:               0

5초 후 시작합니다...
45a5154f11b53d55aadfdf7f958e2fce6a964824: decentraland/catalyst-content에서 가져오는 중
다이제스트: sha256:d6c2981c57cb9367fdb3228d15b07e4d26e62c902944115937de24ecd3ab6aac
상태: decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824에 대해 이미지가 최신입니다
docker.io/decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
45a5154f11b53d55aadfdf7f958e2fce6a964824: decentraland/catalyst-lambdas에서 가져오는 중
다이제스트: sha256:5605812dd29316afc32e561d2a2ce1311f164f07ef3cd99a1cd60727518636e5
상태: decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824에 대해 이미지가 최신입니다
docker.io/decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
latest: decentraland/catalyst-lighthouse에서 가져오는 중
다이제스트: sha256:fedc10b714823909f0a1c17955eed2f22a02e4f784090848d3253ef734e410d4
상태: decentraland/catalyst-lighthouse:latest에 대해 이미지가 최신입니다
docker.io/decentraland/catalyst-lighthouse:latest
latest: decentraland/archipelago-service에서 가져오는 중
다이제스트: sha256:e2c1d6fa96a5cfbbf6fb37e3840c0724fb5abeef8c366025a28fab3ad33d6480
상태: quay.io/decentraland/archipelago-service:latest에 대해 이미지가 최신입니다
quay.io/decentraland/archipelago-service:latest
latest: decentraland/explorer-bff에서 가져오는 중
다이제스트: sha256:fa9e305c44972a8613b01f4cd573d6fb84b0fb03ce953a506fbb06053202913c
상태: quay.io/decentraland/explorer-bff:latest에 대해 이미지가 최신입니다
quay.io/decentraland/explorer-bff:latest
nginx 중지 중 ... 완료
## CATALYST_URL이 http://localhost로 설정되어 있으므로 HTTP를 사용합니다
## nginx 서버 파일에서 $katalyst_host 값을 교체 중... [ OK ]
## 컨테이너 재시작 중...
기본 드라이버로 네트워크 "catalyst-owner_default"를 생성하는 중
catalyst-owner_comms-server_1 생성 중 ... 완료
node-exporter 생성 중                   ... 완료
catalyst-owner_lambdas_1 생성 중      ... 완료
postgres 생성 중                      ... 완료
catalyst-owner_certbot_1 생성 중        ... 완료
nats 생성 중                            ... 완료
postgres-exporter 생성 중               ... 완료
catalyst-owner_content-server_1 생성 중 ... 완료
catalyst-owner_archipelago_1 생성 중    ... 완료
catalyst-owner_explorer-bff_1 생성 중   ... 완료
nats-exporter 생성 중                   ... 완료
cadvisor 생성 중                        ... 완료
nginx 생성 중                           ... 완료
## Catalyst 서버가 http://localhost 에서 실행 중입니다
```

모든 것이 잘 되었다면, 이제 완전한 Decentraland 노드가 실행 중입니다. 이제 브라우저를 열고 URL을 입력하면 됩니다. 기본값을 사용했다면, [`http://localhost`](http://localhost/).

다음 docker 명령으로 콘텐츠 서버 로그를 확인할 수 있습니다:

```bash
> docker logs catalyst-owner_content-server_1
```

서버는 지금 실행 중이지만, 아직 완전히 운영 준비가 된 것은 아닙니다. 다른 [DAO 서버들](https://decentraland.github.io/catalyst-monitor) 의 콘텐츠를 동기화해야 같은 경험을 사용자에게 제공할 수 있습니다. 동기화에는 오랜 시간이 걸릴 수 있습니다. 좋은 인터넷 연결에서도 6시간 정도가 흔합니다. 그러니… 커피 한 잔, 낮잠, 충분한 숙면을 하고 내일 다시 오세요.

동기화가 완료되었는지 확인하는 한 가지 방법은 콘텐츠 상태 엔드포인트의 결과를 확인하는 것입니다: <http://localhost/content/status> (여기에 자신의 URL을 사용하세요).

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153514917,
    "synchronizationState": "Bootstrapping"
  },
  "snapshot": {
    "entities": {
      "profile": 1271331,
      "scene": 23477,
      "wearable": 17351,
      "store": 890
    },
    "lastUpdatedTime": 1658153414530
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

일 때 `synchronizationState` 가 `Bootstrapping`인 동안에는 노드가 새 배포 수락을 일시적으로 중단합니다. 이 조치는 노드가 DAO 네트워크에서 최신 상태로 올라오기 전까지 새로운 엔티티가 배포되지 않도록 보장합니다. 상태가 다음으로 바뀌면 `Syncing`이라고 표시되며, 이는 노드가 성공적으로 따라잡았고 이제 최신 업데이트를 계속 받고 있음을 의미합니다. 이것이 노드가 완전히 동작하며 새 배포를 수락하는 정상 상태입니다.

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153872301,
    "synchronizationState": "Syncing"
  },
  "snapshot": {
    "entities": {
      "profile": 1271317,
      "scene": 23477,
      "store": 890,
      "wearable": 17350
    },
    "lastUpdatedTime": 1658152050030
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

CLI를 더 선호하는 사람이라면, 다음 메시지를 기준으로 콘텐츠 서버 docker 이미지 로그를 grep할 수 있습니다: `서버 포인터 변경 사항에서 엔티티 동기화를 시작하는 중`. 이 텍스트가 보이면 콘텐츠 서버는 완전히 동기화되었고 전부 사용해도 됩니다.

### 환경 변수

다음은 시작 시 콘텐츠 서버 로그에 기록되는 기본값과 함께 모든 콘텐츠 서버 환경 변수를 포괄적으로 정리한 목록입니다:

```jsx
STORAGE_ROOT_FOLDER: "/app/storage/content_server/"
DENYLIST_FILE_NAME: "denylist.txt"
DENYLIST_URLS: "https://config.decentraland.org/denylist"
SYNC_IGNORED_ENTITY_TYPES: ""
FOLDER_MIGRATION_MAX_CONCURRENCY: 1000
SERVER_PORT: 6969
LOG_REQUESTS: false
UPDATE_FROM_DAO_INTERVAL: 1800000
SYNC_WITH_SERVERS_INTERVAL: 45000
CHECK_SYNC_RANGE: 1200000
DECENTRALAND_ADDRESS: "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b"
DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL: 60
DEPLOYMENTS_DEFAULT_RATE_LIMIT_MAX: 300
ETH_NETWORK: "mainnet"
LOG_LEVEL: "debug"
FETCH_REQUEST_TIMEOUT: "2m"
USE_COMPRESSION_MIDDLEWARE: false
BOOTSTRAP_FROM_SCRATCH: false
REQUEST_TTL_BACKWARDS: 1200000
LAND_MANAGER_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/land-manager"
COLLECTIONS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-ethereum-mainnet"
COLLECTIONS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-matic-mainnet"
THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/tpr-matic-mainnet"
BLOCKS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-ethereum-mainnet"
BLOCKS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-matic-mainnet"
PSQL_DATABASE: "content"
PSQL_HOST: "postgres"
PSQL_SCHEMA: "public"
PSQL_PORT: "5432"
GARBAGE_COLLECTION: true
GARBAGE_COLLECTION_INTERVAL: 21600000
PG_IDLE_TIMEOUT: 30000
PG_QUERY_TIMEOUT: 60000
PG_STREAM_QUERY_TIMEOUT: 600000
SNAPSHOT_FREQUENCY_IN_MILLISECONDS: 21600000
CUSTOM_DAO: undefined
DISABLE_SYNCHRONIZATION: false
SYNC_STREAM_TIMEOUT: "10m"
CONTENT_SERVER_ADDRESS: "http://localhost/content/"
REPOSITORY_QUEUE_MAX_CONCURRENCY: 50
REPOSITORY_QUEUE_MAX_QUEUED: 300
REPOSITORY_QUEUE_TIMEOUT: "1m"
ENTITIES_CACHE_SIZE: 150000
DEPLOYMENT_RATE_LIMIT_MAX: {}
DEPLOYMENT_RATE_LIMIT_TTL: {}
VALIDATE_API: false
RETRY_FAILED_DEPLOYMENTS_DELAY_TIME: 900000
```

### 프로덕션에서 노드 사용하기

자신의 서버를 운영해 네트워크 확장에 기여하고 싶으시다면, 우선 정말 멋진 일이며 커뮤니티와 재단은 그렇게 해 주시는 데 큰 감사를 드립니다.

이 경우 네트워크에 참여하기 위한 승인을 DAO에 요청하는 절차를 거쳐야 하며, 다음을 방문하면 됩니다. [이 링크](https://governance.decentraland.org/submit/catalyst/)입니다. 또한 인프라와 관리 비용을 충당하기 위해 MANA 보조금을 요청할 수도 있습니다.

서버는 Decentraland에 들어오는 데 커뮤니티의 누구나 사용할 것이므로, 하드웨어 사양이 위의 하드웨어 요구 사항에서 제시한 수준에 더 가깝게 맞춰져 있는 것이 중요합니다.

### API 사양

콘텐츠, lambdas, comms 서버의 API에 대한 더 자세한 내용은 다음을 확인해 보세요. [API 사양](https://decentraland.github.io/catalyst-api-specs).

### 문제 해결/FAQ

서버를 시작하는 동안 몇 가지 문제가 발생할 수 있습니다. 다음은 시간이 지나며 가장 흔히 겪는 몇 가지 문제와 그 해결 방법입니다.

#### 포트 5432는 로컬 postgres가 사용 중

개발용으로 사용하는 머신에서 노드를 실행하거나 이미 다른 서비스를 호스팅하고 있다면, 그 머신에서 postgres 데이터베이스 서버가 이미 실행 중일 가능성이 높습니다. 따라서 포트 5432가 이미 사용 중일 것입니다.

이 문제를 가장 쉽게 해결하는 방법은 로컬에서 이미 실행 중인 postgres를 중지한 다음 다음을 사용해 postgres docker 컨테이너를 다시 시작하는 것입니다. `docker start postgres`.

docker compose에 대한 지식이 있다면, 다음을 가지고 이것저것 조정해 볼 수 있습니다. `docker-compose.yml` 과 환경 변수를 수정하여 다른 포트에서 동작하도록 시도할 수 있습니다. 여기서는 간단하게 유지하기 위해 이 튜토리얼에서 그 경로는 피하겠습니다.

#### 포트 80은 로컬 nginx가 사용 중

포트 80(심지어 443도)에서 같은 일이 발생할 수 있습니다. 로컬 머신에서 nginx를 실행하고 있다면 포트 80/443이 이미 그 서비스에 바인딩되어 있을 가능성이 높으므로 Catalyst Docker 컨테이너가 이를 사용할 수 없습니다.

역시 가장 간단한 방법은 로컬 nginx 서비스를 중지한 다음 다음을 사용해 docker 컨테이너를 다시 시작하는 것입니다. `docker start nginx`. 또는 대안으로 다음을 활용해야 합니다. `docker-compose.yml` 및 `.env` 를 사용해 다른 포트에서 동작하도록 해야 합니다.

### 참고용 추가 문서

Catalyst 서버가 무엇을 하는지, 어떤 서비스가 포함되는지 등을 더 잘 이해하고 싶다면 각 부분을 자세히 설명하는 아키텍처 저장소 <https://github.com/decentraland/architecture> 를 확인해 보세요.

Catalyst 서버 코드에 기여하고 싶다면 다음을 꼭 확인하세요. [기여 가이드](https://github.com/decentraland/catalyst/blob/main/docs/CONTRIBUTING.md) 개발 프로세스, 버그 수정 및 개선 사항을 제안하는 방법, 변경 사항을 빌드하고 테스트하는 방법을 알아보세요.

마지막으로 팀에 연락해야 한다면 다음을 통해 할 수 있습니다. [Discord](https://discord.com/channels/417796904760639509/948230185457696820) 또는 다음을 제출하여 [GitHub 이슈](https://github.com/decentraland/catalyst/issues).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/tutorials/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
