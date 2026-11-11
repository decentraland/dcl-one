# Escribir pruebas

Las pruebas se pueden escribir de varias maneras. Esta sección intenta establecer un estándar y una justificación detrás de la forma en que escribimos las pruebas.

Las pruebas DEBEN escribirse usando el **describe** y el **it** métodos proporcionados por jest

## Describir y construir contexto

El **describe** El método DEBERÍA usarse para describir, siempre que sea posible, el contexto en el que se ejecutará la parte de código a probar y DEBERÍA construir ese contexto dentro de su ámbito.

```tsx
describe('when the flag is red', () => {
	...
})

describe('when the flag is blue', () => {
	...
})
```

Para describir el contexto, DEBERÍAMOS usar una de las siguientes palabras: **cuando** para describes de nivel superior y **teniendo** o **y** para indicar que existe un contexto que depende de otro contexto que fue definido antes. La justificación detrás de esto es tener contextos autodescritos que puedan entenderse y verificarse fácilmente combinando las sentencias de describe. Como la idea es concatenar las descripciones de los contextos, **describes** DEBEN escribirse en minúsculas.

```tsx
describe('when the flag is red', () => {
	...
	describe('and the wind is strong', () => {
     // El contexto aquí está descrito por la conjunción de los describes:
     // 'cuando la bandera es roja y el viento es fuerte'
     ...
  })
})
```

Se pueden anidar múltiples describes en el mismo nivel y en uno más bajo. Los describes en el mismo nivel denotan contextos diferentes y los describes en niveles inferiores (describes anidados) denotan un contexto más específico.

```tsx
describe('when the flag is red', () => {
	...
	describe('and the wind is strong', () => {
     ...
  })

	describe('and the wind is weak', () => {
     ...
  })
})
```

Como se mencionó antes, cada describe DEBERÍA ir acompañado de un método que construya el contexto o de un método que destruya o cambie el contexto. Los métodos que DEBEN usarse son **beforeEach** y el **afterEach**.


> [!WARNING]
> El framework Jest tiene dos métodos, **beforeEach** y **beforeAll**, pero combinarlos a ambos genera confusión y problemas de ejecución cuando hay contextos anidados presentes, ya que [el orden en el que se ejecutan](https://jestjs.io/docs/setup-teardown#scoping) no es el que esperaríamos.


Los contextos creados con el **beforeEach** método estarán cada vez más definidos a medida que profundizamos en el árbol de describe. Cada **beforeEach** especificará más el contexto según lo que diga su texto.

### Ámbito de variables

El **variables** establecidas en los contextos DEBEN definirse de acuerdo con el ámbito en el que se van a usar. Es decir, si una variable se usa solo para un contexto específico en un nivel anidado de describes, la variable DEBE definirse solo en ese contexto específico. La justificación detrás de esto es mantener las variables cerca del código en el que se usan, haciendo que el código sea más fácil de entender.

```tsx
let flag = 'blue'
describe('when the flag is red', () => {
	let wind = 'mild'
	beforeEach(() => {
    flag = 'red'
  })
  ...

	describe('and the wind is strong', () => {
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
```

Variables de **tipos primitivos**, que **no cambiarán** en el contexto en el que se definen o en contextos posteriores DEBEN definirse como **const** una vez, en lugar de la **beforeEach** definición en el ámbito.

Variables de **tipos primitivos** que cambiarán en el contexto en el que se definen o en contextos posteriores DEBEN definirse como **let**, como la naturaleza de Typescript lo exige.

Variables de **tipos no primitivos** (objetos, arrays, etc.) DEBEN definirse como **let** y su valor DEBE establecerse en un **beforeEach.** La justificación detrás de esto es que los objetos en JS son mutables, lo que significa que cualquier ejecución de código que use ese objeto está sujeta a cambios y puede afectar a otras pruebas de manera involuntaria.

```tsx
let flag
describe('when the flag is red', () => {
  // Un valor primitivo que va a modificarse en múltiples ámbitos.
	let wind
  // Un valor de objeto que se va a usar en el código de la prueba.
  let someObject
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// Un valor primitivo constante que se va a usar solo en este ámbito.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

Las variables DEBERÍAN estar correctamente tipadas siempre que sea posible. Los tipos repetidos DEBERÍAN abstraerse en un tipo.

```tsx
let flag: string
describe('when the flag is red', () => {
  // Un valor primitivo que va a modificarse en múltiples ámbitos.
	let wind: string
  // Un valor de objeto que se va a usar en el código de la prueba.
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// Un valor primitivo constante que se va a usar solo en este ámbito.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

## Describir expectativas y ejecutar código

El **it** el método DEBE colocarse siempre dentro del ámbito de un **describe** y DEBE usarse para describir qué es lo que esperamos del código que vamos a probar y DEBERÍA contener, si es posible, solo **una** aserción. Múltiples aserciones por **it** pueden existir si hay un problema de rendimiento, ya que las pruebas se ejecutarán una vez por **it** (debido al **beforeEach**), o si lo que se espera puede describirse con claridad en la descripción de la expectativa.

```tsx
let flag: string
describe('when the flag is red', () => {
	let wind: string
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1, swim: jest.fn() }
  })
  ...

  // Una expectativa única de la ejecución de una prueba en un contexto (un it)
	describe('and the wind is strong', () => {
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    
		it('should not go swimming', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(false)
    })
  })

  // Múltiples expectativas de la ejecución de una prueba en un contexto (dos its)
	describe('and the wind is weak', () => {
    let result: boolean
		beforeEach(() => {
	    wind = 'weak'
      result = goSwimming(flag, wind, someObject)
	  })

		it('should go swimming', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(true)
    })

    it('should have called the swim method', () => {
      expect(someObject.swim).toHaveBeenCalled()
    })
  })
})
...
```

La justificación detrás de esta estructura es aportar claridad al revisor de las pruebas y a los desarrolladores que mantendrán y harán cambios al código, ya que cada contexto y expectativa está claramente enumerado, lo que facilita entender qué se está probando, cómo se está probando y qué falta por probar.

Siguiendo las **describe** descripciones hasta los **it**s los desarrolladores pueden entender fácilmente lo que se está probando concatenando las oraciones.

```tsx
// Esto se lee como: 'cuando la bandera es roja y el viento es fuerte, 
// debe ir a nadar'
describe('when the flag is red', () => {
  ...
  describe('and the wind is strong', () => {
		...
		it('should go swimming', () => {
			...
		})
  })
})
```

### Escribir expectativas claras

La descripción de las expectativas escrita en los **it**s DEBE ser lo más descriptiva posible acerca de lo que se espera de la ejecución de las pruebas. Los desarrolladores NO DEBEN usar frases abstractas o generales para definir expectativas, ya que esto quita claridad sobre lo que se espera del código.


> [!CAUTION]
> **El desarrollador NO DEBE usar frases como:**
> 
> * 'should work as expected' ⇒ ¿Qué es lo que debería funcionar como se espera?
> * 'should return the correct value' ⇒ ¿Cuál es el valor correcto?
> * 'should resolve/return/work correctly' ⇒ ¿Cómo hace algo para funcionar correctamente?
> * 'should fail' ⇒ ¿Cómo debería fallar? ¿Qué mensaje debería proporcionar?


Debe tenerse en cuenta que, al especificar lo que se espera en los **it**s o cuál será el contexto en los **describe**s los desarrolladores PUEDEN usar nombres de funciones o mensajes de error exactos si es necesario para comprender mejor la intención, pero DEBERÍAN usar una representación textual cuando sea posible para facilitar el mantenimiento de las pruebas.

```tsx
// math.ts
export function div(a: number, b: number): number {
	if(b === 0)	{
		throw new Error('The divisor b equals 0, the division can\'t be performed')
	}
}

// test.spec.ts
import { div } from './math.ts'

describe('when dividing by zero', () => {
  // La descripción del it describe la intención del mensaje de la excepción
	it('should throw an exception signaling that a division by 0 is not possible', () => {
		expect(() => div(12, 0)).toThrowError('The divisor b equals 0, the division can\'t be performed')
	})
})
```

## Qué probar

Elegir qué probar puede variar dependiendo del código que se esté ejecutando. Diferentes factores, desde el rendimiento o un gran dominio de entradas, pueden cambiar definitivamente qué se debe o no se debe probar en las pruebas. Aquí expondremos un único conjunto de casos que el desarrollador DEBERÍA probar si es posible.

```tsx
function run(kilometers: number): number {
	if(kilometers > 1000) {
    throw new Error('The runner can\'t run more than 1000 kilometers')
  }

  if(kilometers <= 10) {
		return kilometers
  } else if(kilometers > 10) {
		doSomething(kilometers)
    return beLazy(kilometers)
  }
}
```

La función run aquí tiene un par de flujos de ejecución diferentes. Las funciones, o secciones de código, NO DEBEN probarse basándose solo en los diferentes flujos de ejecución posibles, sino en lo que se espera de la función, ya que la función podría no estar haciendo para lo que fue escrita y las pruebas podrían estar estrechamente acopladas al código y no detectar diferentes problemas con él. Esto implica que el desarrollador DEBERÍA probar siempre todas las rutas de ejecución posibles y también DEBERÍA probar otras rutas posibles de acuerdo con la semántica de la función.

Para este caso específico, el desarrollador DEBERÍA probar **al menos** los siguientes casos:

1. Ejecutar la función run con una cantidad de kilómetros mayor que 1000
2. Ejecutar la función run con una cantidad de kilómetros igual a 10
3. Ejecutar la función run con una cantidad de kilómetros mayor que 10 pero menor que 1000

Para el primer caso, el desarrollador necesita probar que la función run lanza una excepción. **Las excepciones DEBEN comprobarse por su mensaje de error**, ya que también podría lanzarse cualquier otra excepción, invalidando el propósito de la prueba. Si la excepción es una excepción personalizada, el desarrollador PUEDE comprobar la instancia de la excepción.

Para el segundo caso, el desarrollador necesita probar que la función devolvió la misma cantidad de kilómetros que se le dio.

Para el tercer caso, el desarrollador necesita probar que la función devolvió lo que una función externa, **beLazy** devolvió y, como **doSomething** (también ejecutada en el método) no puede comprobarse a través del valor de retorno de la función, el desarrollador DEBERÍA **probar que se llamó con los argumentos correctos**.

```tsx
import { doSomething, beLazy } from '../runningUtils'
jest.mock('../runningUtils')

// Nota que no hay un describe principal
// El único contexto global son las funciones que queremos mockear con jest

const mockDoSomething = doSomething as jest.MockedFunction<typeof doSomething>
const mockBeLazy = beLazy as jest.MockedFunction<typeof beLazy>

describe('when running more than 1000 kilometers', () => {
	it('should throw an error signaling that the runner can't run more than 1000 kilometers', () => {
		expect(() => run(1001)).toThrowError("The runner can't run more than 1000 kilometers")
	})
})

describe('when running less or equal than 10', () => {
	it('should return the same amount of kilometers as the ones given', () => {
		expect(run(2)).toEqual(2)
	})
})

describe('when running more than 10 kilometers but less than 1000', () => {
	const kilometers = 50
	let result: number
	beforeEach(() => {
		mockDoSomething.mockReturnValueOnce(undefined)
		mockBeLazy.mockImplementationOnce(value => value)
		result = run(kilometers)
	})
	
	it('should return the kilometers after being lazy', () => {
		expect(result).toEqual(kilometers)
	})

	it('should call the doSomething function with the given kilometers', () => {
		expect(mockDoSomething).toHaveBeenCalledWith(kilometers)
	})
})
```

## Cuándo simular

La simulación dependerá en gran medida del tipo de pruebas que esté escribiendo el desarrollador.

* Las pruebas unitarias DEBERÍAN tener simulada toda función externa, con la excepción de funciones que hagan cosas simples, como formatear cadenas, etc.
* Las pruebas de API DEBERÍAN tener mocks solo para comunicarse con servicios externos, es decir, DBs u otras APIs diferentes. Si una de las operaciones que se realizan en la prueba afecta al rendimiento de la suite de pruebas, se podría implementar un mock para mitigar este problema.

Todos los mocks DEBEN hacerse usando las herramientas proporcionadas por Jest, con la excepción de los `redux-saga-test-plan` mocks.

### Mocks y funciones de utilidad

* Cualquier mock de Jest DEBERÍA hacerse usando su **once** método cuando sea posible, es decir, `mockReturnValueOnce` o `mockResolvedValueOnce` se recomienda evitar `mockReturnValue` o `mockResolvedValue`La justificación detrás de esto es evitar que se ejecuten mocks no deseados, cambiando la ejecución de nuestras pruebas.
* Jest ofrece una serie de tipos que puedes usar para distintos tipos de mocks. Por ejemplo, al hacer mock de un objeto DEBERÍAS usar `jest.Mocked<typeof someObject>`, para clases DEBERÍAS usar `jest.MockedClass<typeof SomeClass>`, y para funciones, `jest.MockedFunction<typeof someFunction>`.
* Un `afterEach` DEBERÍA escribirse para ejecutarse después de cada prueba con un `jest.resetAllMocks` para limpiar cualquier implementación mockeada de módulos mockeados globalmente que pueda filtrarse a otras pruebas. Puedes omitir este reseteo si estás usando `mockReturnValueOnce` o `mockResolvedValueOnce` porque se realiza automáticamente por ti.
* Se puede crear un directorio llamado `mocks` para almacenar mocks, que deberían colocarse en el directorio `test` o `spec` . Los mocks grandes DEBEN almacenarse en archivos distintos de las pruebas para evitar tener archivos de pruebas extensos. Los mocks pequeños, si no hay muchos, DEBERÍAN mantenerse en las pruebas.
  * Los archivos de mock DEBERÍAN nombrarse como la entidad que están simulando, así que, por ejemplo, si estamos simulando un perfil, el mock debería colocarse en el archivo `/test/mocks/profile.ts` archivo.
  * En caso de que se requieran varios mocks grandes, DEBERÍAN colocarse en archivos diferentes dentro de un directorio llamado como la entidad que se va a simular, así que, por ejemplo, si tenemos dos mocks de perfil, los almacenaremos como: `/test/mocks/profile/profile-with-wearables.ts` y `/test/mocks/profile/profile-without-wearables.ts`. Para acceder a ellos fácilmente, DEBERÍAS crear un `index.ts` archivo dentro del `/test/mocks/profile/` directorio y exportarlos.
  * Para evitar la mutación de objetos mockeados, los mocks **DEBEN exportarse como funciones** que los devuelvan.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
