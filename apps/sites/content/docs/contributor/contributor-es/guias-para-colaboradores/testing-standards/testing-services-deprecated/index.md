# Pruebas de servicios (obsoleto)


> [!WARNING]
> Este es un enfoque obsoleto para probar servicios. Para nuevos servicios, consulte [Servicios de prueba (Well-Known-Component)](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/).


Los servicios DEBERÍAN probarse principalmente usando **pruebas de API**, es decir, un conjunto de pruebas que ejercitan el código del servicio a través de su API levantando el servicio, con sus dependencias externas simuladas (DBs y APIs). **Prueba unitaria** DEBERÍA incluirse en los casos en que se encuentren secciones de código utilitario con un alto número de casos que deben ejercitarse o código no eficiente, lo que hace imposible o difícil probarlo con pruebas de API. La justificación de esto es que, debido a nuestra arquitectura de microservicios, que reduce las responsabilidades y, por tanto, la cantidad de código que se va a probar, la lógica de los servicios es simple, lo que hace que las pruebas de API sean la opción con mejor relación tiempo-valor al garantizar que los servicios funcionen correctamente.

## Pila de pruebas

Los servicios DEBEN probarse usando [Jest](https://jestjs.io/) como el framework principal de pruebas con [supertest](https://github.com/visionmedia/supertest) como el framework para levantar el servidor y probarlo.

## Estructura de directorios

Como los servicios PUEDEN tener tanto pruebas de API como pruebas unitarias, habrá más de un lugar donde se colocarán las pruebas:

* Las pruebas unitarias DEBEN ir junto al archivo o módulo que están probando, con el mismo nombre, pero con la extensión `spec.ts` en lugar de `ts`.
* Las pruebas de API ejercitarán código a lo largo de todo el servicio, por lo que DEBEN colocarse en un directorio `test` , en la ruta raíz. Los archivos se nombrarán como el recurso del servicio que van a probar; es decir, si las pruebas de API van a probar el recurso `books`, normalmente bajo `/books`, el nombre del archivo DEBE ser `books.spec.ts` y DEBE colocarse en el directorio `test` .

## Qué probar

* Los middlewares aplicados a la ruta (auth, etc.), para garantizar que la ruta funcione como se espera.
* La lógica de los controladores, para garantizar que la ruta haga lo que esperamos que haga.

## Servicios de prueba de API

Como se mencionó antes, las pruebas de API se harán usando Jest y supertest.

Para ilustrar cómo un desarrollador haría pruebas de API, tenemos el siguiente ejemplo:

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

Encontramos en el ejemplo una ruta con distintos métodos HTTP, GET y POST. Ambos endpoints hacen referencia al mismo recurso, books, por lo que debe crearse un archivo, llamado `books.spec.ts` DEBE crearse debajo del `test` directorio para guardar las pruebas para ellos.

Todas las rutas y los diferentes flujos de ejecución DEBEN probarse exhaustivamente. Estos son los casos que el desarrollador DEBE probar para estos endpoints:

* Solicitar todos los libros funciona, respondiendo con un 200 y la lista de todos los libros.
* Solicitar todos los libros de un autor dado funciona, respondiendo con un 200 y la lista de todos los libros de ese autor.
* Publicar un libro sin autenticación falla, respondiendo con un 401 y un mensaje de error.
* Publicar un libro autenticado funciona, respondiendo con un 200 e insertando el libro.

Estos casos definitivamente tendrán sus **it**s y la expectativa de esos its DEBE contener al menos **el estado de respuesta esperado** y lo que es **esperado en el cuerpo de la respuesta** (si es que hay alguno).

Así es como deberían escribirse estas pruebas de endpoint:

```tsx
import db from '../db'
import app from '../express_app.ts'
// Solo simulamos dependencias externas como las DBs
jest.mock('../db')

const mockDb = db as jest.Mocked<typeof db>

const server = supertest(app.getApp())

// Esto también podría escribirse como /books?
describe('cuando se solicita el recurso books', () => {
	let url: string
	let aBook: Book
	let anotherBook: Book
	beforeEach(() => {
		url = '/books'
		aBook = { title: 'aTitle', author: 'anAuthor' }
		anotherBook = { title: 'anotherTitle', author: 'anotherAuthor' }
	})
	
	describe('cuando se consultan los libros sin autor', () => {
		beforeEach(() => {
			mockDb.getBooks.mockResolvedValueOnce([aBook, anotherBook])
		})

		it('debería responder con un 200 y todos los libros', () => {
			return server
	      .get(url)
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([aBook, anotherBook])
	      })
		})
	})
	
	describe('cuando se consultan los libros con un autor', () => {
		beforeEach(() => {
			mockDb.getFilteredBooks.mockResolvedValueOnce([anotherBook])
		})

		it('debería responder con un 200 y todos los libros de un autor', () => {
			return server
	      .get(url)
				.query({ author: 'anotherAuthor' })
	      .expect(200)
	      .then((response) => {
					expect(response.body).toEqual([anotherBook])
	      })
		})
	})
	
	describe('cuando se publica un nuevo libro sin autenticación', () => {
		it('debería responder con un 401 y un error que diga que el usuario no está autenticado', () => {
			return server
	      .post(url)
				.send(book)
	      .expect(401)
	      .then((response) => {
					expect(response.body).toEqual({ error: 'No autenticado' })
	      })
		})
	})
	
	describe('cuando se publica un nuevo libro autenticado', () => {	
		it('debería responder con un 200 e insertar el libro', () => {
			return server
	      .post('/books')
	      .set(createAuthHeaders('post', url))
				.send(aBook)
	      .expect(200)
	      .then(() => {
					// Comprueba que el libro fue insertado
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
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-deprecated.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
