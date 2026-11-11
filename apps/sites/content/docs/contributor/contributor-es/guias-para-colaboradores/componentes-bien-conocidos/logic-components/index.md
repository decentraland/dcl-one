# Componentes de lógica


> [!NOTE]
> Esta sección se está desarrollando actualmente. Vuelve pronto para obtener documentación completa sobre los componentes de lógica.


Los componentes de lógica son piezas de software que contienen la **lógica de negocio** de tu aplicación. Se organizan semánticamente por dominio o funcionalidad y sirven como capa de orquestación entre controladores y adaptadores.

## Propósito

Componentes de lógica:

* Implementar reglas de negocio y lógica de dominio
* Orquestar operaciones entre múltiples adaptadores
* Encapsular flujos de trabajo complejos
* Permanecer independientes de las preocupaciones de la capa de transporte (HTTP, WebSocket)
* Pueden probarse exhaustivamente con pruebas unitarias sin dependencias de E/S

## Ubicación

Los componentes de lógica DEBEN ubicarse bajo `src/logic` o `src/components` directorio, organizado por dominio:

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

Los componentes de lógica son los consumidores principales de adaptadores. Los usan para interactuar con recursos externos mientras se centran en las reglas de negocio.

```tsx
export function createUserLogic(
  components: Pick<AppComponents, 'database' | 'cache' | 'logs'>
): IUserLogic {
  const { database, cache, logs } = components
  const logger = logs.getLogger('user-logic')

  async function getUserProfile(userId: string): Promise<UserProfile> {
    // Comprobar primero la caché
    const cached = await cache.get(`user:${userId}`)
    if (cached) {
      return cached
    }

    // Obtener de la base de datos
    const user = await database.query('SELECT * FROM users WHERE id = $1', [userId])
    
    // Aplicar la lógica de negocio
    const profile = transformUserToProfile(user)
    
    // Guardar el resultado en caché
    await cache.set(`user:${userId}`, profile, { ttl: 3600 })
    
    return profile
  }

  return {
    getUserProfile
  }
}
```

### 2. Reglas de negocio

Los componentes de lógica hacen cumplir las reglas de negocio y la validación:

```tsx
async function createUser(userData: CreateUserInput): Promise<User> {
  // Regla de negocio: el nombre de usuario debe ser único
  const existing = await database.query(
    'SELECT id FROM users WHERE username = $1',
    [userData.username]
  )
  
  if (existing.rows.length > 0) {
    throw new UserAlreadyExistsError(userData.username)
  }

  // Regla de negocio: validar los datos del usuario
  if (!isValidEmail(userData.email)) {
    throw new InvalidEmailError(userData.email)
  }

  // Crear el usuario
  const user = await database.query(
    'INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *',
    [userData.username, userData.email]
  )

  logger.info('Usuario creado', { userId: user.id })
  
  return user
}
```

### 3. Orquestación del flujo de trabajo

Los componentes de lógica coordinan operaciones complejas de varios pasos:

```tsx
async function publishContent(
  userId: string,
  content: ContentInput
): Promise<PublishedContent> {
  // Paso 1: validar permisos
  const canPublish = await permissions.canUserPublish(userId)
  if (!canPublish) {
    throw new UnauthorizedError('El usuario no puede publicar contenido')
  }

  // Paso 2: procesar el contenido
  const processed = await processContent(content)

  // Paso 3: almacenar el contenido
  const stored = await storage.save(processed)

  // Paso 4: actualizar índices
  await searchIndex.index(stored)

  // Paso 5: notificar a los suscriptores
  await notifications.notifySubscribers(userId, stored)

  logger.info('Contenido publicado', { contentId: stored.id, userId })

  return stored
}
```

## Buenas prácticas

### 1. Responsabilidad única

Cada componente de lógica debe centrarse en un dominio o contexto acotado:

```tsx
// Bien: centrado en el dominio de usuario
createUserLogic()
createContentLogic()
createPermissionsLogic()

// Evitar: responsabilidades mezcladas
createUserAndContentLogic()
```

### 2. Inyección de dependencias

Inyecta siempre las dependencias a través del parámetro components:

```tsx
export function createOrderLogic(
  components: Pick<AppComponents, 'database' | 'payments' | 'inventory' | 'logs'>
): IOrderLogic {
  // Usar las dependencias inyectadas
}
```

### 3. Manejo de errores

Lanza errores de dominio significativos que los controladores puedan capturar y manejar:

```tsx
// Definir errores específicos del dominio
export class InsufficientInventoryError extends Error {
  constructor(public readonly productId: string, public readonly requested: number) {
    super(`Inventario insuficiente para el producto ${productId}. Solicitado: ${requested}`)
    this.name = 'InsufficientInventoryError'
  }
}
```

### 4. Lógica de negocio pura

Mantén los componentes de lógica libres de preocupaciones de la capa de transporte:

```tsx
// Bien: lógica de negocio pura
async function calculateOrderTotal(items: OrderItem[]): Promise<number> {
  return items.reduce((total, item) => total + item.price * item.quantity, 0)
}

// Evitar: preocupaciones de la capa HTTP/transporte
async function calculateOrderTotal(req: Request, res: Response): Promise<void> {
  // No hagas esto en los componentes de lógica
}
```

## Pruebas de los componentes de lógica

Los componentes de lógica deben probarse exhaustivamente con pruebas unitarias. Consulta la [documentación de Testing Services (WKC)](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/) documentación para más detalles.

```tsx
describe('cuando se crea un usuario', () => {
  let userLogic: IUserLogic
  let mockDatabase: jest.Mocked<IDatabase>

  beforeEach(() => {
    mockDatabase = createMockDatabase()
    userLogic = createUserLogic({ database: mockDatabase, logs: mockLogs })
  })

  describe('y el nombre de usuario ya existe', () => {
    beforeEach(() => {
      mockDatabase.query.mockResolvedValueOnce({ rows: [{ id: '123' }] })
    })

    it('debería lanzar UserAlreadyExistsError', async () => {
      await expect(
        userLogic.createUser({ username: 'existing', email: 'test@test.com' })
      ).rejects.toThrow(UserAlreadyExistsError)
    })
  })
})
```

## Próximamente

Esta sección se ampliará con:

* Ejemplos detallados de componentes de lógica orientados al dominio
* Patrones para manejar flujos de trabajo complejos
* Directrices para organizar una lógica de dominio amplia
* Integración con sistemas de eventos
* Estrategias de caché
* Patrones de gestión de transacciones


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/componentes-bien-conocidos/logic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
