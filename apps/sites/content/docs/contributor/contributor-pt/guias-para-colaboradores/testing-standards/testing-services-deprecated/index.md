# Testar Services (obsoleto)


> [!WARNING]
> Esta é uma abordagem obsoleta para testar serviços. Para novos serviços, consulte [Serviços de Teste (Well-Known-Component)](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc/).


Os services DEVEM ser testados principalmente usando **testes de API**, ou seja, um conjunto de testes que exercitam o código do serviço por meio da sua API, colocando o serviço no ar, com as suas dependências externas sendo mockadas (DBs e APIs). **teste unitário** DEVEM ser incluídos em casos em que sejam encontradas seções utilitárias de código com um alto número de casos a serem exercitados ou código não performático, tornando impossível ou difícil testá-lo com testes de API. A justificativa para isso é que, devido à nossa arquitetura de microservices, que reduz as responsabilidades e, portanto, a quantidade de código a ser testado, a lógica dos serviços é simples, tornando os testes de API a opção com melhor relação tempo/valor ao garantir que os serviços funcionem corretamente.

## Stack de testes

Os serviços DEVEM ser testados usando [Jest](https://jestjs.io/) como o principal framework de testes com [supertest](https://github.com/visionmedia/supertest) como o framework para configurar o servidor e testá-lo.

## Estrutura de diretórios

Como os serviços PODEM ter tanto testes de API quanto testes unitários, haverá mais de um lugar para onde os testes irão:

* Os testes unitários DEVEM ficar ao lado do arquivo ou módulo que estão testando, com o mesmo nome, mas com a extensão `spec.ts` em vez de `ts`.
* Os testes de API exercitarão código por todo o serviço, então DEVEM ser colocados em um `test` diretório, no caminho raiz. Os arquivos serão nomeados com o recurso do serviço que estarão testando, ou seja, se os testes de API estiverem testando o recurso `books`, normalmente em `/books`, o nome do arquivo DEVE ser `books.spec.ts` e DEVE ser colocado no `test` diretório.

## O que testar

* Os middlewares aplicados à rota (auth, etc.), para garantir que a rota funcione como esperado.
* A lógica dos controllers, para garantir que a rota faça o que esperamos que ela faça.

## Testando serviços de API

Como mencionado antes, os testes de API serão feitos usando Jest e supertest.

Para ilustrar como um desenvolvedor fará testes de API, temos o seguinte exemplo:

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

Encontramos no exemplo uma rota com diferentes métodos HTTP, GET e POST. Ambos os endpoints se referem ao mesmo recurso, books, então um arquivo, chamado `books.spec.ts` DEVE ser criado no `test` diretório para armazenar os testes para eles.

Todas as rotas e os diferentes fluxos de execução DEVEM ser testados minuciosamente. Estes são os casos que o desenvolvedor DEVE testar para estes endpoints:

* Solicitar todos os livros funciona, respondendo com um 200 e a lista de todos os livros.
* Solicitar todos os livros de um determinado autor funciona, respondendo com um 200 e a lista de todos os livros desse autor.
* Postar um livro sem autenticação falha, respondendo com um 401 e uma mensagem de erro.
* Postar um livro autenticado funciona, respondendo com um 200 e inserindo o livro.

Esses casos certamente terão os seus **it**s e a expectativa desses its DEVE conter pelo menos **o status de resposta esperado** e o que é **esperado no corpo da resposta** (se houver algum).

É assim que estes testes de endpoint devem ser escritos:

```tsx
import db from '../db'
import app from '../express_app.ts'
// Nós apenas mockamos dependências externas como DBs
jest.mock('../db')

const mockDb = db as jest.Mocked<typeof db>

const server = supertest(app.getApp())

// Isto também pode ser escrito como /books?
describe('quando solicitar o recurso books', () => {
	let url: string
	let aBook: Book
	let anotherBook: Book
	beforeEach(() => {
		url = '/books'
		aBook = { title: 'aTitle', author: 'anAuthor' }
		anotherBook = { title: 'anotherTitle', author: 'anotherAuthor' }
	})
	
	describe('quando consultar os livros sem um autor', () => {
		beforeEach(() => {
			mockDb.getBooks.mockResolvedValueOnce([aBook, anotherBook])
		})

		it('deve responder com um 200 e todos os livros', () => {
			return server
	      .get(url)
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([aBook, anotherBook])
	      })
		})
	})
	
	describe('quando consultar os livros com um autor', () => {
		beforeEach(() => {
			mockDb.getFilteredBooks.mockResolvedValueOnce([anotherBook])
		})

		it('deve responder com um 200 e todos os livros de um autor', () => {
			return server
	      .get(url)
				.query({ author: 'anotherAuthor' })
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([anotherBook])
	      })
		})
	})
	
	describe('quando postar um novo livro sem autenticação', () => {
		it('deve responder com um 401 e um erro dizendo que o usuário não está autenticado', () => {
			return server
	      .post(url)
				.send(book)
	      .expect(401)
	      .then((response) => {
					expect(response.body).toEqual({ error: 'Unauthenticated' })
	      })
		})
	})
	
	describe('quando postar um novo livro autenticado', () => {	
		it('deve responder com um 200 e inserir o livro', () => {
			return server
	      .post('/books')
	      .set(createAuthHeaders('post', url))
				.send(aBook)
	      .expect(200)
	      .then(() => {
					// Verifica que o livro foi inserido
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
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-deprecated.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
