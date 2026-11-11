# 测试服务（已弃用）


> [!WARNING]
> 这是用于测试服务的弃用方法。对于新服务，请参阅 [测试服务（知名组件）](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc/).


服务应主要使用 **API 测试**，也就是说，一组通过启动服务并将其外部依赖进行模拟（数据库和 API）来经由其 API 运行服务代码的测试。 **单元测试** 应在发现需要大量用例覆盖的工具性代码段，或发现性能较差、无法或难以通过 API 测试进行测试的代码时纳入。其背后的原因在于，由于我们的微服务架构会减少各自职责，从而减少需要测试的代码量，服务逻辑较为简单，因此在确保服务正确工作时，API 测试是单位时间价值最高的选择。

## 测试栈

服务必须使用 [Jest](https://jestjs.io/) 作为主要测试框架，并配合 [supertest](https://github.com/visionmedia/supertest) 作为用于搭建服务器并对其进行测试的框架。

## 目录结构

由于服务可能同时包含 API 测试和单元测试，因此测试会放在不止一个位置：

* 单元测试必须与其所测试的文件或模块放在一起，名称相同，但扩展名为 `spec.ts` 而不是 `ts`.
* API 测试会覆盖整个服务中的代码，因此它们必须放在根路径下的 `test` 目录中。文件名应与它们要测试的服务资源相同，也就是说，如果 API 测试要测试的资源是 `books`，通常位于 `/books`，那么文件名必须是 `books.spec.ts` 并且必须放在 `test` 目录中。

## 测试内容

* 应用于路由的中间件（认证等），以确保路由按预期工作。
* 控制器逻辑，以确保路由执行我们期望的操作。

## API 测试服务

如前所述，API 测试将使用 Jest 和 supertest 进行。

为了说明开发者如何编写 API 测试，下面是一个示例：

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

在这个示例中，我们看到同一路由使用了不同的 HTTP 方法，GET 和 POST。两个端点都指向同一个资源 books，因此需要创建一个名为 `books.spec.ts` 的文件，并且必须创建在 `test` 目录下，用于存放它们的测试。

所有路由以及不同的执行流程都必须彻底测试。开发者必须为这些端点测试以下情况：

* 请求所有书籍可正常工作，返回 200 和所有书籍列表。
* 按给定作者请求所有书籍可正常工作，返回 200 和该作者的所有书籍列表。
* 未认证情况下发布书籍会失败，返回 401 和错误信息。
* 认证后发布书籍可正常工作，返回 200 并插入该书。

这些情况肯定会有它们的 **其**s 以及这些项的期望必须至少包含 **预期的响应状态** 以及 **响应体中预期的内容** （如果有的话）。

这些端点测试应这样编写：

```tsx
import db from '../db'
import app from '../express_app.ts'
// 我们只模拟外部依赖，例如数据库
jest.mock('../db')

const mockDb = db as jest.Mocked<typeof db>

const server = supertest(app.getApp())

// 这也可以写成 /books?
describe('当请求 books 资源时', () => {
	let url: string
	let aBook: Book
	let anotherBook: Book
	beforeEach(() => {
		url = '/books'
		aBook = { title: 'aTitle', author: 'anAuthor' }
		anotherBook = { title: 'anotherTitle', author: 'anotherAuthor' }
	})
	
	describe('当查询不带作者的 books 时', () => {
		beforeEach(() => {
			mockDb.getBooks.mockResolvedValueOnce([aBook, anotherBook])
		})

		it('应返回 200 和所有书籍', () => {
			return server
	      .get(url)
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([aBook, anotherBook])
	      })
		})
	})
	
	describe('当按作者查询 books 时', () => {
		beforeEach(() => {
			mockDb.getFilteredBooks.mockResolvedValueOnce([anotherBook])
		})

		it('应返回 200 和该作者的所有书籍', () => {
			return server
	      .get(url)
				.query({ author: 'anotherAuthor' })
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([anotherBook])
	      })
		})
	})
	
	describe('当未认证发布新书时', () => {
		it('应返回 401，并给出用户未认证的错误信息', () => {
			return server
	      .post(url)
				.send(book)
	      .expect(401)
	      .then((response) => {
					expect(response.body).toEqual({ error: 'Unauthenticated' })
	      })
		})
	})
	
	describe('当已认证发布新书时', () => {	
		it('应返回 200 并插入该书', () => {
			return server
	      .post('/books')
	      .set(createAuthHeaders('post', url))
				.send(aBook)
	      .expect(200)
	      .then(() => {
					// 检查该书已被插入
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
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-deprecated.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
