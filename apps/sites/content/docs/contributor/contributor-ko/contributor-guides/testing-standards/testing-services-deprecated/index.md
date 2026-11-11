# 서비스 테스트 (사용 중단)


> [!WARNING]
> 서비스 테스트를 위한 더 이상 사용되지 않는 방식입니다. 새 서비스의 경우 다음을 참조하세요 [테스트 서비스(Well-Known-Component)](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc/).


서비스는 주로 다음을 사용해 테스트해야 합니다 **API 테스트**, 즉 외부 의존성(DB와 API)을 목(mock) 처리한 상태로 서비스를 기동한 뒤, API를 통해 서비스 코드를 실행하는 테스트 집합입니다. **단위 테스트** 코드의 유틸리티 섹션 중 실행해야 할 케이스가 많거나 성능이 좋지 않은 코드가 발견되어 API 테스트로는 테스트하기 불가능하거나 어려운 경우에 포함되어야 합니다. 그 근거는 마이크로서비스 아키텍처로 인해 책임 범위가 줄어들어 테스트해야 할 코드의 양이 적어지고, 서비스의 로직이 단순해져 서비스가 올바르게 동작하는지 확인할 때 API 테스트가 시간 대비 가치가 가장 높은 선택이 되기 때문입니다.

## 테스트 스택

서비스는 다음을 사용해 테스트해야 합니다 [Jest](https://jestjs.io/) 을 기본 테스트 프레임워크로 사용하고 [supertest](https://github.com/visionmedia/supertest) 를 서버를 설정하고 테스트하는 프레임워크로 사용합니다.

## 디렉터리 구조

서비스는 API 테스트와 단위 테스트를 모두 가질 수 있으므로, 테스트가 위치할 곳은 한 곳 이상일 것입니다:

* 단위 테스트는 테스트 대상 파일이나 모듈과 같은 이름을 사용하되, 확장자는 다음으로 하여 그 옆에 두어야 합니다 `spec.ts` 대신 `ts`.
* API 테스트는 서비스 전반의 코드를 실행하므로, 다음에 배치해야 합니다 `test` 디렉터리를 루트 경로에 두어야 합니다. 파일 이름은 테스트할 서비스의 리소스 이름으로 지정해야 하므로, 예를 들어 API 테스트가 리소스 `books`를 테스트하는 경우 보통 `/books` 아래에 있으며, 파일 이름은 반드시 다음이어야 합니다 `books.spec.ts` 이어야 하며 반드시 다음에 배치되어야 합니다 `test` 디렉터리.

## 무엇을 테스트할 것인가

* 라우트에 적용된 미들웨어(auth 등)가 라우트가 예상대로 동작하는지 확인합니다.
* 컨트롤러 로직이 우리가 기대하는 대로 라우트가 동작하는지 확인합니다.

## API 테스트 서비스

앞서 언급했듯이, API 테스트는 Jest와 supertest를 사용해 수행합니다.

개발자가 API 테스트를 어떻게 작성하는지 보여주기 위해 다음 예시를 제공합니다:

```tsx
import express from "express"
import { authMiddleware } from "../auth"
import db from "../db"

const app = express()

app.get('/books', async (req, res) => {
	let books: Books[];
	if(req.query.author) {
		books = await db.getFilteredBooks({ author: req.query.author })
	} else {
		books = await db.getBooks()
	}
  res.json(books)
})

app.post('/books', authMiddleware, async (req, res) => {
	await db.insertBook(req.body)
	res.status(200).end()
})
```

예시에서 서로 다른 HTTP 메서드인 GET과 POST를 가진 하나의 라우트를 확인할 수 있습니다. 두 엔드포인트는 같은 리소스인 books를 참조하므로, 다음과 같은 이름의 파일이 `books.spec.ts` 반드시 `test` 디렉터리 아래에 생성되어 그 테스트를 담아야 합니다.

모든 라우트와 다양한 실행 흐름은 철저히 테스트해야 합니다. 개발자가 이 엔드포인트들에 대해 테스트해야 하는 경우는 다음과 같습니다:

* 모든 책을 요청하면 200과 모든 책 목록을 응답해야 합니다.
* 특정 저자의 모든 책을 요청하면 200과 해당 저자의 모든 책 목록을 응답해야 합니다.
* 인증되지 않은 상태에서 책을 게시하면 실패해야 하며, 401과 오류 메시지를 응답해야 합니다.
* 인증된 상태에서 책을 게시하면 성공해야 하며, 200을 응답하고 책을 삽입해야 합니다.

이 경우에는 반드시 해당 **it**들과 그 it들에 대한 기대값에는 최소한 다음이 포함되어야 합니다 **예상되는 응답 상태** 그리고 무엇이 **응답 본문에서 예상되는지** (있다면).

이러한 엔드포인트 테스트는 다음과 같이 작성해야 합니다:

```tsx
import db from '../db'
import app from '../express_app.ts'
// 외부 의존성은 DB와 같은 것만 목 처리합니다
jest.mock('../db')

const mockDb = db as jest.Mocked<typeof db>

const server = supertest(app.getApp())

// 이것은 /books? 로도 작성할 수 있습니다
describe('책 리소스를 요청할 때', () => {
	let url: string
	let aBook: Book
	let anotherBook: Book
	beforeEach(() => {
		url = '/books'
		aBook = { title: 'aTitle', author: 'anAuthor' }
		anotherBook = { title: 'anotherTitle', author: 'anotherAuthor' }
	})
	
	describe('저자 없이 책을 조회할 때', () => {
		beforeEach(() => {
			mockDb.getBooks.mockResolvedValueOnce([aBook, anotherBook])
		})

		it('200과 모든 책을 응답해야 합니다', () => {
			return server
	      .get(url)
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([aBook, anotherBook])
	      })
		})
	})
	
	describe('저자와 함께 책을 조회할 때', () => {
		beforeEach(() => {
			mockDb.getFilteredBooks.mockResolvedValueOnce([anotherBook])
		})

		it('200과 해당 저자의 모든 책을 응답해야 합니다', () => {
			return server
	      .get(url)
				.query({ author: 'anotherAuthor' })
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([anotherBook])
	      })
		})
	})
	
	describe('인증되지 않은 상태에서 새 책을 게시할 때', () => {
		it('401과 사용자가 인증되지 않았다는 오류를 응답해야 합니다', () => {
			return server
	      .post(url)
				.send(book)
	      .expect(401)
	      .then((response) => {
					expect(response.body).toEqual({ error: '인증되지 않음' })
	      })
		})
	})
	
	describe('인증된 상태에서 새 책을 게시할 때', () => {	
		it('200을 응답하고 책을 삽입해야 합니다', () => {
			return server
	      .post('/books')
	      .set(createAuthHeaders('post', url))
				.send(aBook)
	      .expect(200)
	      .then(() => {
					// 책이 삽입되었는지 확인합니다
	        expect(db.insertBook).toHaveBeenCalledWith(aBook)
	      })
		})
	})
})
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-deprecated.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
