# Logic Components


> [!NOTE]
> Esta secção está atualmente a ser desenvolvida. Volte em breve para obter documentação abrangente sobre componentes lógicos.


Os componentes lógicos são peças de software que contêm a **lógica de negócio** da sua aplicação. Estão organizados semanticamente por domínio ou funcionalidade e servem como camada de orquestração entre controladores e adaptadores.

## Propósito

Componentes lógicos:

* Implementam regras de negócio e lógica de domínio
* Orquestram operações entre vários adaptadores
* Encapsulam fluxos de trabalho complexos
* Mantêm-se independentes das preocupações da camada de transporte (HTTP, WebSocket)
* Podem ser exaustivamente testados com testes unitários sem dependências de I/O

## Localização

Os componentes lógicos DEVEM ser colocados em `src/logic` ou `src/components` diretório, organizados por domínio:

```
src/
└── logic/
    ├── users/
    ├── content/
    ├── permissions/
    └── notifications/
```

## Características

### 1. Consumo de adaptadores

Os componentes lógicos são os principais consumidores de adaptadores. Utilizam adaptadores para interagir com recursos externos, enquanto se concentram nas regras de negócio.

```tsx
export function createUserLogic(
  components: Pick<AppComponents, 'database' | 'cache' | 'logs'>
): IUserLogic {
  const { database, cache, logs } = components
  const logger = logs.getLogger('user-logic')

  async function getUserProfile(userId: string): Promise<UserProfile> {
    // Verificar primeiro a cache
    const cached = await cache.get(`user:${userId}`)
    if (cached) {
      return cached
    }

    // Obter da base de dados
    const user = await database.query('SELECT * FROM users WHERE id = $1', [userId])
    
    // Aplicar lógica de negócio
    const profile = transformUserToProfile(user)
    
    // Guardar o resultado em cache
    await cache.set(`user:${userId}`, profile, { ttl: 3600 })
    
    return profile
  }

  return {
    getUserProfile
  }
}
```

### 2. Regras de negócio

Os componentes lógicos impõem regras de negócio e validação:

```tsx
async function createUser(userData: CreateUserInput): Promise<User> {
  // Regra de negócio: o nome de utilizador tem de ser único
  const existing = await database.query(
    'SELECT id FROM users WHERE username = $1',
    [userData.username]
  )
  
  if (existing.rows.length > 0) {
    throw new UserAlreadyExistsError(userData.username)
  }

  // Regra de negócio: validar dados do utilizador
  if (!isValidEmail(userData.email)) {
    throw new InvalidEmailError(userData.email)
  }

  // Criar o utilizador
  const user = await database.query(
    'INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *',
    [userData.username, userData.email]
  )

  logger.info('Utilizador criado', { userId: user.id })
  
  return user
}
```

### 3. Orquestração do fluxo de trabalho

Os componentes lógicos coordenam operações complexas com vários passos:

```tsx
async function publishContent(
  userId: string,
  content: ContentInput
): Promise<PublishedContent> {
  // Passo 1: Validar permissões
  const canPublish = await permissions.canUserPublish(userId)
  if (!canPublish) {
    throw new UnauthorizedError('O utilizador não pode publicar conteúdo')
  }

  // Passo 2: Processar conteúdo
  const processed = await processContent(content)

  // Passo 3: Armazenar conteúdo
  const stored = await storage.save(processed)

  // Passo 4: Atualizar índices
  await searchIndex.index(stored)

  // Passo 5: Notificar subscritores
  await notifications.notifySubscribers(userId, stored)

  logger.info('Conteúdo publicado', { contentId: stored.id, userId })

  return stored
}
```

## Melhores práticas

### 1. Responsabilidade única

Cada componente lógico deve focar-se num domínio ou contexto delimitado:

```tsx
// Bom: Focado no domínio do utilizador
createUserLogic()
createContentLogic()
createPermissionsLogic()

// Evitar: responsabilidades misturadas
createUserAndContentLogic()
```

### 2. Injeção de dependências

Injete sempre dependências através do parâmetro components:

```tsx
export function createOrderLogic(
  components: Pick<AppComponents, 'database' | 'payments' | 'inventory' | 'logs'>
): IOrderLogic {
  // Usar dependências injetadas
}
```

### 3. Tratamento de erros

Lance erros de domínio significativos que os controladores possam apanhar e tratar:

```tsx
// Definir erros específicos do domínio
export class InsufficientInventoryError extends Error {
  constructor(public readonly productId: string, public readonly requested: number) {
    super(`Insufficient inventory for product ${productId}. Requested: ${requested}`)
    this.name = 'InsufficientInventoryError'
  }
}
```

### 4. Lógica de negócio pura

Mantenha os componentes lógicos livres de preocupações da camada de transporte:

```tsx
// Bom: Lógica de negócio pura
async function calculateOrderTotal(items: OrderItem[]): Promise<number> {
  return items.reduce((total, item) => total + item.price * item.quantity, 0)
}

// Evitar: preocupações da camada HTTP/transporte
async function calculateOrderTotal(req: Request, res: Response): Promise<void> {
  // Não faça isto em componentes lógicos
}
```

## Testar Componentes Lógicos

Os componentes lógicos devem ser exaustivamente testados com testes unitários. Consulte a [Serviços de teste (WKC)](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc/) documentação para mais detalhes.

```tsx
describe('ao criar um utilizador', () => {
  let userLogic: IUserLogic
  let mockDatabase: jest.Mocked<IDatabase>

  beforeEach(() => {
    mockDatabase = createMockDatabase()
    userLogic = createUserLogic({ database: mockDatabase, logs: mockLogs })
  })

  describe('e o nome de utilizador já existe', () => {
    beforeEach(() => {
      mockDatabase.query.mockResolvedValueOnce({ rows: [{ id: '123' }] })
    })

    it('deve lançar UserAlreadyExistsError', async () => {
      await expect(
        userLogic.createUser({ username: 'existing', email: 'test@test.com' })
      ).rejects.toThrow(UserAlreadyExistsError)
    })
  })
})
```

## Em breve

Esta secção será expandida com:

* Exemplos detalhados de componentes lógicos orientados ao domínio
* Padrões para lidar com fluxos de trabalho complexos
* Diretrizes para organizar a lógica de domínio em grande escala
* Integração com sistemas de eventos
* Estratégias de cache
* Padrões de gestão de transações


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/well-known-components/logic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
