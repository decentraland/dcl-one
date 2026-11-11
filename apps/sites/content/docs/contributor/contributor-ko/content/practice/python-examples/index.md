# Python 예제

이 실습에서는 다음을 사용해 일부 콘텐츠를 다운로드하고 분석하는 간단한 프로그램을 작성하는 방법을 보여줍니다. [스냅샷](/docs/contributor/contributor-ko/content/snapshots/) 콘텐츠 서버에서 제공됩니다.


> [!NOTE]
> 다음에서 찾을 수 있습니다. [전체 스크립트](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) GitHub에서, 또한 다음과 함께 [더 고급 예제](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


우리는 Decentraland Foundation의 서버인 다음 주소를 사용할 것입니다. `peer.decentraland.org`, 그리고 언어로 Python 3를 사용할 것입니다.

우리가 할 작업은 다음과 같습니다:

1. 콘텐츠 서버의 상태를 조회합니다.
2. 선택하고 다운로드할 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/) 엔티티 목록이 포함된
3. 참조된 모든 엔티티의 유형과 ID를 출력합니다.

몇 가지 준비 작업으로 스크립트를 시작해 봅시다. 표준 라이브러리 모듈만 사용하겠지만, 실제 코드에서는 아마 더 편한 HTTP 클라이언트(예: 다음과 같은 [requests](https://github.com/psf/requests) 라이브러리).

```py
# HTTP GET 요청을 보내고, 파일처럼 다룰 수 있는 HTTP 응답을 반환합니다.
def fetch(path):
    url = f"https://peer.decentraland.org/{path}"
    headers = { "User-Agent": "urllib" } # 일부 서버에서 중요합니다(비어 있으면 403 Forbidden)

    request = urllib.request.Request(url, headers=headers)
    response = urllib.request.urlopen(request)

    return response
```

우리의 간단한 헬퍼는 HTTP `GET` 요청을 보내고, 파일처럼 다룰 수 있는 응답 객체를 반환합니다. 별것 아닙니다. 이를 사용해 다음에 요청을 보내고 `/about` 엔드포인트에 접근해 서버 상태를 확인해 봅시다:

```py
# 서버 상태 확인:
about = json.load(fetch('about'))

if not about['healthy']:
    print("서버가 정상적이지 않습니다!")
    sys.exit(1)
```

이 지점을 지나면 서버가 정상적으로 실행 중인 것입니다(우리는 `200` 응답을 받았고) 정상 동작 중이라고 보고합니다. 현재 스냅샷 집합을 요청할 수 있습니다( JSON 배열 형식으로 제공됩니다):

```py
# 스냅샷 목록을 가져옵니다:
all_snapshots = json.load(fetch('content/snapshots'))
```

스냅샷 파일은(특히 더 긴 시간 범위의 경우) 매우 클 수 있습니다. 간단한 실험을 위해 목록에서 가장 작은 것을 다음 기준으로 골라 봅시다. `numberOfEntities`:

```py
# 포함된 엔티티 수 기준으로 가장 작은 스냅샷을 선택합니다:
snapshot = min(all_snapshots, key=lambda s: s['numberOfEntities'])
```

콘텐츠를 다운로드하려면 다음이 필요합니다 `해시` 의 필드 `스냅샷`. 콘텐츠 루트에 이를 덧붙여 파일 URL을 얻습니다:

```py
# 콘텐츠 API에서 파일을 요청합니다:
response = fetch('content/contents/' + snapshot['hash'])
```

선택한 파일은 메모리에 버퍼링할 만큼 작지만, 그 사실을 모른다고 가정하고 스트리밍해 봅시다. 첫 번째 줄은 스냅샷 헤더이고, 그 이후의 각 줄에는 JSON 객체가 들어 있습니다.

헤더를 확인해 봅시다. 언제나 좋은 생각입니다:

```py
# 스냅샷 헤더를 검증합니다:
header = response.readline().decode('utf-8').strip()

if header != '### Decentraland json snapshot':
    print("잘못된 스냅샷 헤더: " + header)
    sys.exit(1)
```

이제 응답을 한 줄씩 읽으면서 스냅샷의 모든 엔티티를 처리할 수 있습니다. 이 예제의 소박한 목적에서는, *처리한다는 것은* 엔티티 유형과 ID를 출력한다는 뜻입니다:

```py
# 모든 항목을 한 줄당 JSON 하나씩 읽고 디코딩합니다:
for line in response:
    item = json.loads(line)
    print(item['entityType'], item['entityId'])
```

이 루프는 스냅샷 처리가 끝날 때까지 다음과 같은 줄들을 스트리밍하고, 파싱하고, 출력합니다:

```
프로필 bafkreic36qmzyprs6whkpuxbeiif4no6kvdrr2tfpichbx2fzfz5py6eyv
씬 bafkreibr5xfujqrp5q3o4s73vm2yljlcp7cucqgugssnarsuclxv4emlmy
프로필 bafkreid7khr5wnkialba44rsslffi633rh3lvctad5oa5vjoe6wa7s4c5a
웨어러블 bafkreihlqcb7jgubomyidikpwpqhgzbagltk5m4rgbjdvzydxmoka7bg4i
```

건배! 우리는 스냅샷 시스템을 사용해 Decentraland에서 사용 가능한 일부 콘텐츠를 살펴보았습니다.

기억하세요, 다음에서 찾을 수 있습니다 [전체 스크립트](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) GitHub에서, 또한 다음과 함께 [더 고급 예제](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/practice/python-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
