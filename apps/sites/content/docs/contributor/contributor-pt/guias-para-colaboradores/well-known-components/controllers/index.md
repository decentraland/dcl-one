# Controladores

Os controladores são responsáveis por lidar com solicitações HTTP e WebSocket na sua aplicação. Eles servem como ponto de entrada para solicitações externas e atuam como a ponte entre a camada de transporte e a sua lógica de negócio.

## Propósito

Os controladores DEVEM executar apenas estas tarefas específicas:

1. **Validar a entrada da camada de transporte** - Verificar autenticação, autorização e formato de entrada
2. **Chamar componentes de lógica** - Delegar a lógica de negócio aos componentes de lógica apropriados
3. **Transformar entrada** - Converter os dados da solicitação em parâmetros para chamadas aos componentes de lógica
4. **Tratar erros** - Capturar e responder adequadamente aos erros dos componentes de lógica


> [!WARNING]
> Os controladores não devem conter lógica de negócio. Todas as regras de negócio DEVEM ser implementadas em componentes de lógica.


## Localização

Todos os controladores DEVEM ser colocados no `/src/controllers` diretório e nomeados em referência à ação que executam.

```
src/
└── controllers/
    ├── users/
    │   ├── get-user.ts
    │   ├── create-user.ts
    │   └── update-user.ts
    ├── content/
    │   ├── publish-content.ts
    │   └── delete-content.ts
    └── health.ts
```

## Estrutura do controlador

### Manipulador HTTP básico

```tsx
import { HandlerContextWithPath } from '@dcl/platform-server-commons'

export async function getUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/:id'>
) {
  const { 
    components: { userLogic, logs },
    params: { id },
    verification 
  } = context

  const logger = logs.getLogger('get-user-handler')

  try {
    // 1. Validar autenticação
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Autenticação obrigatória'
        }
      }
    }

    // 2. Validar entrada
    if (!id || typeof id !== 'string') {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'ID de usuário inválido'
        }
      }
    }

    // 3. Chamar o componente de lógica
    const user = await userLogic.getUser(id)

    // 4. Retornar resposta de sucesso
    return {
      status: 200,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    // 5. Tratar erros
    logger.error('Erro ao buscar usuário', error)

    if (error instanceof UserNotFoundError) {
      return {
        status: 404,
        body: {
          error: 'Not Found',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'Ocorreu um erro inesperado'
      }
    }
  }
}
```

### Manipulador de solicitação POST

```tsx
export async function createUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users'>
) {
  const {
    components: { userLogic, logs },
    request,
    verification
  } = context

  const logger = logs.getLogger('create-user-handler')

  try {
    // 1. Validar autenticação
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Autenticação obrigatória'
        }
      }
    }

    // 2. Analisar e validar o corpo da solicitação
    const body = await request.json()
    
    if (!body.username || !body.email) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'Nome de usuário e email são obrigatórios'
        }
      }
    }

    // 3. Transformar a entrada para o componente de lógica
    const userData = {
      username: body.username.toLowerCase(),
      email: body.email.toLowerCase(),
      display_name: body.display_name || body.username
    }

    // 4. Chamar o componente de lógica
    const user = await userLogic.createUser(userData)

    // 5. Retornar resposta de sucesso
    return {
      status: 201,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    logger.error('Erro ao criar usuário', error)

    if (error instanceof UserAlreadyExistsError) {
      return {
        status: 409,
        body: {
          error: 'Conflict',
          message: error.message
        }
      }
    }

    if (error instanceof InvalidEmailError) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'Ocorreu um erro inesperado'
      }
    }
  }
}
```

### Manipulador de parâmetros de consulta

```tsx
export async function searchUsersHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/search'>
) {
  const {
    components: { userLogic, logs },
    url,
    verification
  } = context

  const logger = logs.getLogger('search-users-handler')

  try {
    // 1. Validar autenticação
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Autenticação obrigatória'
        }
      }
    }

    // 2. Extrair e validar os parâmetros de consulta (minúsculas!)
    const searchParams = new URLSearchParams(url.search)
    const query = searchParams.get('query')?.toLowerCase()
    const limit = parseInt(searchParams.get('limit') || '10', 10)
    const offset = parseInt(searchParams.get('offset') || '0', 10)

    if (!query) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'O parâmetro de consulta é obrigatório'
        }
      }
    }

    // 3. Chamar o componente de lógica
    const results = await userLogic.searchUsers({
      query,
      limit,
      offset
    })

    // 4. Retornar resposta de sucesso
    return {
      status: 200,
      body: {
        ok: true,
        data: results
      }
    }
  } catch (error) {
    logger.error('Erro ao buscar usuários', error)

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'Ocorreu um erro inesperado'
      }
    }
  }
}
```

## Requisitos de entrada

Todas as chaves de entrada recebidas por meio de controladores (corpo JSON, parâmetros de consulta, parâmetros de URL, cabeçalhos, etc.) **DEVEM estar em minúsculas** para evitar problemas de maiúsculas/minúsculas ao processar dados.

### Parâmetros com várias palavras

Parâmetros com várias palavras DEVEM ser definidos usando **snake\_case**.


> [!CAUTION]
> **Nomeação incorreta:**
> 
> * `minPrice`
> * `Carro`
> * `somethingURL`
> * `userId`



> [!TIP]
> **Nomeação correta:**
> 
> * `min_price`
> * `carro`
> * `something_url`
> * `user_id`


### Exemplo

```tsx
// Corpo da solicitação
{
  "user_name": "john_doe",        // ✅ Correto
  "email_address": "john@example.com",  // ✅ Correto
  "display_name": "John Doe",     // ✅ Correto
  "max_items": 100                // ✅ Correto
}

// Parâmetros de consulta
?search_query=test&max_results=50&sort_by=created_at  // ✅ Correto

// Parâmetros de URL
/users/:user_id/posts/:post_id   // ✅ Correto
```

## Tratamento de erros

### Respostas de erro padrão

Os controladores devem retornar formatos consistentes de resposta de erro:

```tsx
// 400 Solicitação inválida
{
  "error": "Solicitação inválida",
  "message": "Mensagem de erro detalhada",
  "details": {} // Detalhes adicionais opcionais
}

// 401 Não autorizado
{
  "error": "Não autorizado",
  "message": "Autenticação obrigatória"
}

// 403 Proibido
{
  "error": "Proibido",
  "message": "Você não tem permissão para acessar este recurso"
}

// 404 Não encontrado
{
  "error": "Não encontrado",
  "message": "Recurso não encontrado"
}

// 409 Conflito
{
  "error": "Conflito",
  "message": "O recurso já existe"
}

// 500 Erro interno do servidor
{
  "error": "Erro interno do servidor",
  "message": "Ocorreu um erro inesperado"
}
```

### Mapeamento de erros

Mapeie erros de domínio para códigos de status HTTP apropriados:

```tsx
function mapErrorToResponse(error: Error, logger: ILogger) {
  logger.error('Erro do handler', error)

  // Erros específicos do domínio
  if (error instanceof UserNotFoundError) {
    return { status: 404, body: { error: 'Não encontrado', message: error.message } }
  }
  
  if (error instanceof UserAlreadyExistsError) {
    return { status: 409, body: { error: 'Conflito', message: error.message } }
  }
  
  if (error instanceof UnauthorizedError) {
    return { status: 403, body: { error: 'Proibido', message: error.message } }
  }
  
  if (error instanceof InvalidInputError) {
    return { status: 400, body: { error: 'Solicitação inválida', message: error.message } }
  }

  // Erro genérico
  return {
    status: 500,
    body: {
      error: 'Internal Server Error',
      message: 'Ocorreu um erro inesperado'
    }
  }
}
```

## Autenticação e autorização

### Verificação de autenticação

```tsx
// Exigir autenticação
if (!verification?.auth) {
  return {
    status: 401,
    body: {
      error: 'Unauthorized',
      message: 'Autenticação obrigatória'
    }
  }
}

const userAddress = verification.auth.toLowerCase()
```

### Verificação de autorização

```tsx
// Verificar permissões por meio do componente de lógica
const canAccess = await permissionsLogic.canUserAccessResource(
  userAddress,
  resourceId
)

if (!canAccess) {
  return {
    status: 403,
    body: {
      error: 'Forbidden',
      message: 'Você não tem permissão para acessar este recurso'
    }
  }
}
```

## Melhores Práticas

### 1. Mantenha os controladores enxutos

Os controladores devem ser wrappers enxutos em torno dos componentes de lógica:

```tsx
// ✅ Bom: controlador enxuto
export async function handler(context) {
  const { userLogic } = context.components
  const user = await userLogic.getUser(context.params.id)
  return { status: 200, body: { data: user } }
}

// ❌ Ruim: lógica de negócio no controlador
export async function handler(context) {
  const { database } = context.components
  const user = await database.query('SELECT * FROM users...')
  const processed = processUser(user) // Lógica de negócio!
  const validated = validateUser(processed) // Lógica de negócio!
  return { status: 200, body: { data: validated } }
}
```

### 2. Valide cedo

Valide e falhe rapidamente:

```tsx
// Valide a autenticação primeiro
if (!verification?.auth) {
  return { status: 401, body: { error: 'Não autorizado' } }
}

// Depois valide a entrada
if (!params.id) {
  return { status: 400, body: { error: 'Solicitação inválida' } }
}

// Depois prossiga com a lógica de negócio
const result = await logic.doSomething(params.id)
```

### 3. Use segurança de tipos

Aproveite o TypeScript para segurança de tipos:

```tsx
interface CreateUserRequest {
  user_name: string
  email_address: string
  display_name?: string
}

const body: CreateUserRequest = await request.json()
```

### 4. Registre adequadamente

Registre eventos e erros importantes:

```tsx
logger.info('Usuário criado', { userId: user.id, userAddress })
logger.warn('Limite de taxa se aproximando', { userAddress, requests })
logger.error('Falha ao criar usuário', { error, userAddress })
```

## Testando controladores

Veja a [Serviços de teste (WKC)](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc/) documentação para orientações sobre testes de integração.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/well-known-components/controllers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
