# Escrever testes

Os testes podem ser escritos de várias formas. Esta secção tenta fornecer um padrão e uma justificação para a forma como escrevemos testes.

Os testes DEVEM ser escritos usando os **describe** e os **it** métodos fornecidos pelo jest

## Descrição e construção do contexto

O **describe** método DEVE ser usado para descrever, sempre que possível, o contexto em que a parte do código a ser testada será executada e DEVE construir esse contexto dentro do seu escopo.

```tsx
describe('when the flag is red', () => {
	...
})

describe('when the flag is blue', () => {
	...
})
```

Para descrever o contexto, DEVEMOS usar uma das seguintes palavras: **when** para describes de nível superior e **having** ou **e** para indicar que existe um contexto que depende de outro contexto que foi definido antes. A justificação para isto é ter contextos autodescritos que possam ser facilmente compreendidos e verificados ao combinar as frases de describe. Como a ideia é concatenar as descrições dos contextos, **describes** DEVEM ser escritos em minúsculas.

```tsx
describe('when the flag is red', () => {
	...
	describe('and the wind is strong', () => {
     // O contexto aqui é descrito pela conjunção dos describes:
     // "when the red flag is red and the wind is strong"
     ...
  })
})
```

Vários describes podem ser aninhados ao mesmo nível e a um nível inferior. Os describes ao mesmo nível denotam contextos diferentes e os describes em níveis inferiores (describes aninhados) denotam um contexto mais específico.

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

Como mencionado antes, cada describe DEVE ser acompanhado por um método que constrói o contexto ou por um método que destrói ou altera o contexto. Os métodos que DEVEM ser usados são os **beforeEach** e os **afterEach**.


> [!WARNING]
> O framework Jest tem dois métodos, **beforeEach** e **beforeAll**, mas combinar ambos gera confusão e problemas de execução quando existem contextos aninhados, pois [a ordem pela qual são executados](https://jestjs.io/docs/setup-teardown#scoping) não é o que esperaríamos.


Os contextos criados com o **beforeEach** método ficarão cada vez mais definidos à medida que descemos na árvore de describe. Cada **beforeEach** especificará o contexto mais de acordo com o que o seu texto diz.

### Escopo de variáveis

O **variáveis** definidas nos contextos DEVEM ser definidas de acordo com o escopo em que vão ser usadas. Ou seja, se uma variável é usada apenas para um contexto específico num nível aninhado de describes, a variável DEVE ser definida apenas nesse contexto específico. A justificação para isto é manter as variáveis próximas do código em que estão a ser usadas, tornando o código mais fácil de entender.

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

Variáveis de **tipos primitivos**, que **não serão alteradas** no contexto em que são definidas ou em contextos subsequentes DEVEM ser definidas como **const** uma vez, em vez da **beforeEach** definição no escopo.

Variáveis de **tipos primitivos** que serão alterados no contexto em que são definidos ou em contextos subsequentes DEVEM ser definidos como **let**, como a natureza do TypeScript o impõe.

Variáveis de **tipos não primitivos** (objetos, arrays, etc) DEVEM ser definidos como **let** e o seu valor DEVE ser definido num **beforeEach.** A justificação para isto é que os objetos em JS são mutáveis, o que significa que qualquer execução de código que use esse objeto está sujeita a alterações e pode afetar outros testes de forma involuntária.

```tsx
let flag
describe('when the flag is red', () => {
  // Um valor primitivo que vai ser modificado em vários escopos.
	let wind
  // Um valor de objeto que vai ser usado no código do teste.
  let someObject
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// Um valor primitivo constante que vai ser usado apenas neste escopo.
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

As variáveis DEVEM ser tipadas corretamente sempre que possível. Tipos repetidos DEVEM ser abstraídos para um tipo.

```tsx
let flag: string
describe('when the flag is red', () => {
  // Um valor primitivo que vai ser modificado em vários escopos.
	let wind: string
  // Um valor de objeto que vai ser usado no código do teste.
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// Um valor primitivo constante que vai ser usado apenas neste escopo.
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

## Descrever expectativas e executar código

O **it** método DEVE estar sempre colocado dentro do escopo de um **describe** e DEVE ser usado para descrever o que esperamos do código que vamos testar e DEVE conter, se possível, apenas **uma** asserção. Múltiplas asserções por **it** podem existir se houver um problema de desempenho, já que os testes serão executados uma vez por **it** (devido ao **beforeEach**), ou se o que é esperado puder ser descrito com clareza na descrição da expectativa.

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

  // Expectativa única da execução de um teste num contexto (um it)
	describe('and the wind is strong', () => {
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    
		it('should not go swimming', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(false)
    })
  })

  // Múltiplas expectativas da execução de um teste num contexto (dois its)
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

A justificação para esta estrutura é fornecer clareza ao revisor dos testes e aos developers que vão manter e alterar o código, uma vez que cada contexto e expectativa estão claramente enumerados, tornando mais fácil entender o que está a ser testado, como está a ser testado e o que falta testar.

Seguindo as **describe** descrições até aos **it**os developers podem facilmente entender o que está a ser testado ao concatenar as frases.

```tsx
// Isto lê-se como: "when the flag is red and the wind is strong, 
// it should go swimming"
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

### Escrever Expectativas Claras

A descrição das expectativas escritas nos **it**s DEVE ser o mais descritiva possível sobre o que é esperado da execução dos testes. Os developers NÃO DEVEM usar linguagem abstrata ou geral para definir expectativas, pois isso reduz a clareza sobre o que é esperado do código.


> [!CAUTION]
> **O developer NÃO DEVE usar frases como:**
> 
> * "should work as expected" ⇒ O que é que deve funcionar como esperado?
> * "should return the correct value" ⇒ Qual é o valor correto?
> * "should resolve/return/work correctly" ⇒ Como é que algo funciona corretamente?
> * "should fail" ⇒ Como é que deve falhar? Qual é a mensagem que deve fornecer?


Deve ser tido em consideração que, ao especificar o que é esperado nos **it**s ou qual será o contexto nos **describe**s, os developers PODEM usar nomes de funções ou mensagens de erro exatas se isso for necessário para entender melhor a intenção, mas DEVEM usar uma representação textual sempre que possível para tornar os testes mais fáceis de manter.

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
  // A descrição do it descreve a intenção da mensagem da exceção
	it('should throw an exception signaling that a division by 0 is not possible', () => {
		expect(() => div(12, 0)).toThrowError('The divisor b equals 0, the division can\'t be performed')
	})
})
```

## O que testar

A escolha do que testar pode variar consoante o código que está a ser executado. Fatores diferentes, desde desempenho a um grande domínio de entradas, podem certamente alterar o que deve ou não deve ser testado. Aqui vamos expor um conjunto único de casos que o developer DEVE testar, se possível.

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

A função run aqui tem alguns fluxos de execução diferentes. As funções, ou secções de código, NÃO DEVEM ser testadas apenas com base nos diferentes fluxos de execução possíveis, mas sim no que é esperado da função, pois a função pode não estar a fazer o que foi escrita para fazer e os testes podem ficar fortemente acoplados ao código e não detetar problemas diferentes nele. Isto implica que o developer DEVE testar sempre todos os caminhos de execução possíveis e DEVE também testar outros caminhos possíveis de acordo com a semântica da função.

Para este caso específico, o developer DEVE testar **pelo menos** os seguintes casos:

1. Executar a função run com uma quantidade de kilómetros superior a 1000
2. Executar a função run com uma quantidade de kilómetros igual a 10
3. Executar a função run com uma quantidade de kilómetros superior a 10 mas inferior a 1000

Para o primeiro caso, o developer precisa de testar que a função run lança uma exceção. **As exceções DEVEM ser verificadas pela sua mensagem de erro**, pois qualquer outra exceção também poderia ser lançada, invalidando o propósito do teste. Se a exceção for uma exceção personalizada, o developer PODE verificar a instância da exceção.

Para o segundo caso, o developer precisa de testar que a função devolveu o mesmo número de kilómetros que lhe foi dado.

Para o terceiro caso, o developer precisa de testar que a função devolveu o que uma função externa, **beLazy** devolveu e, como **doSomething** (também executada no método) não pode ser verificada através do valor de retorno da função, o developer DEVE **testar que foi chamada com os argumentos corretos**.

```tsx
import { doSomething, beLazy } from '../runningUtils'
jest.mock('../runningUtils')

// Note que não existe um describe principal
// O único contexto global são as funções que queremos simular com jest

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

## Quando fazer mock

Fazer mock dependerá principalmente do tipo de testes que o developer está a escrever.

* Os testes de unidade DEVEM ter todas as funções externas com mock, com exceção das funções que fazem coisas simples, como formatar strings, etc.
* Os testes de API DEVEM ter apenas mocks para comunicar com serviços externos, ou seja, DBs ou APIs diferentes. Se uma das operações feitas no teste afetar o desempenho da suite de testes, um mock pode ser implementado para mitigar esse problema.

Todos os mocks DEVEM ser feitos usando as ferramentas fornecidas pelo Jest, com exceção dos `redux-saga-test-plan` mocks.

### Mocks e funções utilitárias

* Quaisquer mocks do Jest DEVEM ser simulados usando o seu método **once** sempre que possível, ou seja, `mockReturnValueOnce` ou `mockResolvedValueOnce` não é recomendado `mockReturnValue` ou `mockResolvedValue`. A justificação para isto é impedir que mocks indesejados sejam executados, alterando a execução dos nossos testes.
* O Jest oferece uma série de Types que podes usar para diferentes tipos de mocks. Por exemplo, ao simular um objeto, DEVE usar `jest.Mocked<typeof someObject>`, para classes DEVE usar `jest.MockedClass<typeof SomeClass>`, e para funções, `jest.MockedFunction<typeof someFunction>`.
* Um `afterEach` DEVE ser escrito para correr depois de cada teste com um `jest.resetAllMocks` para limpar qualquer implementação simulada de módulos simulados globalmente que possa vazar para outros testes. Podes ignorar este reset se estiveres a usar `mockReturnValueOnce` ou `mockResolvedValueOnce` porque isso é feito automaticamente por ti.
* Um diretório chamado `mocks` PODE ser criado para armazenar mocks, que devem ser colocados no diretório `test` ou `spec` . Mocks grandes DEVEM ser guardados em ficheiros diferentes dos testes para evitar ter ficheiros de teste extensos. Mocks pequenos, se não forem muitos, DEVEM ser mantidos nos testes.
  * Os ficheiros de mock DEVEM ser nomeados de acordo com a entidade que estão a simular, por isso, por exemplo, se estivermos a simular um profile, o mock deve ser colocado no ficheiro `/test/mocks/profile.ts` file.
  * Caso sejam necessários vários mocks grandes, DEVEM ser colocados em ficheiros diferentes dentro de um diretório chamado como a entidade a ser simulada, por exemplo, se tivermos dois mocks de profile, vamos guardá-los como: `/test/mocks/profile/profile-with-wearables.ts` e `/test/mocks/profile/profile-without-wearables.ts`. Para lhes aceder facilmente, DEVE criar um `index.ts` ficheiro dentro do diretório `/test/mocks/profile/` e exportá-los.
  * Para evitar a mutação de objetos simulados, os mocks **DEVEM ser exportados como funções** que os devolvem.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
