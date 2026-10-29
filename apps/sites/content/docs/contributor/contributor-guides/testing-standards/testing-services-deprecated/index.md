# Testing Services (Deprecated)


> [!WARNING]
> This is a deprecated approach for testing services. For new services, please refer to [Testing Services (Well-Known-Component)](/docs/contributor/contributor-guides/testing-standards/testing-services-wkc/).


Services SHOULD be mainly tested using **API tests**, that is, a set of tests that exercise the code of the service through its API by getting the service up, with its external dependencies being mocked (DBs and APIs). **Unit test** SHOULD be included in cases where utility sections of code with a high number of cases to be exercised or non-performant code is found, making it impossible or hard to test with API tests. The rationale behind this is that due to our micro-services architecture, that reduces the responsibilities thus the amount of code to be tested, the logic of the services is simple, making API tests the most time per value option when ensuring that the services work correctly.

## Testing stack

Services MUST be tested using [Jest](https://jestjs.io/) as the main testing framework with [supertest](https://github.com/visionmedia/supertest) as the framework to set up the server and test it.

## Directory structure

As services MAY have both API test and unit tests, there will be more than one place where the tests will go:

* Unit tests MUST go alongside the file or module that they're testing, with the same name, but with the extension `spec.ts` instead of `ts`.
* API tests will exercise code all along the service, so they MUST be placed in a `test` directory, in the root path. The files will be named as the service's resource that they will be testing, that is, if the API tests will be testing the resource `books`, usually under `/books`, the name of the file MUST be `books.spec.ts` and MUST be placed in the `test` directory.

## What to test

* Middlewares applied to the route (auth, etc), to ensure that the route works as expected.
* The controllers logic, to ensure that the route does what we expect it to do.

## API testing services

As mentioned before, API tests will be done using Jest and supertest.

To illustrate how a developer will do API tests, we have the following example:

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

We find in the example one route with different HTTP methods, GET and POST. Both endpoints refer to the same resource, books, so a file, named `books.spec.ts` MUST be created under the `test` directory to hold the tests for them.

All the routes and the different flows of executions MUST be tested throughly. These are the cases that the developer MUST test for these endpoints:

* Requesting all the books works, responding with a 200 and the list of all books.
* Requesting all the books of a given author works, responding with a 200 and the list of all books of that author.
* Posting a book unauthenticated fails, responding with a 401 and an error message.
* Posting a book authenticated works, responding with a 200 and inserting the book.

These cases will definitely get their **it**s and the expectation of those its MUST contain at least **the expected response status** and what is **expected in the response body** (if there's any).

This is how these endpoint tests should be written:

```tsx
import db from '../db'
import app from '../express_app.ts'
// We only mock external dependencies such a DBs
jest.mock('../db')

const mockDb = db as jest.Mocked<typeof db>

const server = supertest(app.getApp())

// This might be also written as /books?
describe('when requesting the books resource', () => {
	let url: string
	let aBook: Book
	let anotherBook: Book
	beforeEach(() => {
		url = '/books'
		aBook = { title: 'aTitle', author: 'anAuthor' }
		anotherBook = { title: 'anotherTitle', author: 'anotherAuthor' }
	})
	
	describe('when querying the books without an author', () => {
		beforeEach(() => {
			mockDb.getBooks.mockResolvedValueOnce([aBook, anotherBook])
		})

		it('should respond with a 200 and all the books', () => {
			return server
	      .get(url)
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([aBook, anotherBook])
	      })
		})
	})
	
	describe('when querying the books with an author', () => {
		beforeEach(() => {
			mockDb.getFilteredBooks.mockResolvedValueOnce([anotherBook])
		})

		it('should respond with a 200 and all the books of an author', () => {
			return server
	      .get(url)
				.query({ author: 'anotherAuthor' })
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([anotherBook])
	      })
		})
	})
	
	describe('when posting a new book unauthenticated', () => {
		it('should respond with a 401 and an error saying that the user is unauthenticated', () => {
			return server
	      .post(url)
				.send(book)
	      .expect(401)
	      .then((response) => {
					expect(response.body).toEqual({ error: 'Unauthenticated' })
	      })
		})
	})
	
	describe('when posting a new book authenticated', () => {	
		it('should respond with a 200 and insert the book', () => {
			return server
	      .post('/books')
	      .set(createAuthHeaders('post', url))
				.send(aBook)
	      .expect(200)
	      .then(() => {
					// Checks that the book was inserted
	        expect(db.insertBook).toHaveBeenCalledWith(aBook)
	      })
		})
	})
})
```
