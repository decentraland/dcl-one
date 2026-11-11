# Adaptadores

Adaptadores são componentes responsáveis por lidar com **processos de E/S** e integrações externas. Eles servem como a ponte entre sua lógica de negócios e sistemas externos, como bancos de dados, APIs, sistemas de arquivos ou armazenamentos de cache.

## Objetivo

Os adaptadores abstraem os detalhes de implementação das dependências externas, permitindo:

* **Intercambiabilidade** - Trocar implementações facilmente (por exemplo, PostgreSQL → MongoDB)
* **Testabilidade** - Simular dependências externas em testes
* **Isolamento** - Manter a lógica de negócios independente de preocupações de infraestrutura

## Localização

Todos os componentes de adaptador DEVEM ser colocados no `src/adapters` diretório do projeto.

```
src/
└── adapters/
    ├── database/
    ├── storage/
    ├── cache/
    └── external-api/
```

## Interfaces compartilhadas

Os adaptadores DEVEM ser construídos pensando na intercambiabilidade. Defina interfaces compartilhadas em `src/types.ts` para que vários adapters possam implementar o mesmo contrato.

### Exemplo: Interface de armazenamento

```tsx
// src/types.ts
export interface IStorage {
  set: (key: string, value: any) => Promise<void>
  get: (key: string) => Promise<any>
  delete: (key: string) => Promise<void>
}
```

## Adaptador de armazenamento em memória

Aqui está uma implementação simples de um adaptador de armazenamento em memória, útil para testes ou desenvolvimento.

### Estrutura de diretórios

```
src/adapters/memory-storage/
├── component.ts
├── types.ts
├── errors.ts
└── index.ts
```

### Implementação: `component.ts`

```tsx
import { IStorage } from '../../types'

export function createMemoryStorageAdapter(): IStorage {
  // Estado interno - limitado a esta instância do componente
  const memory: Record<string, any> = {}

  async function set(key: string, value: any): Promise<void> {
    memory[key] = value
  }

  async function get(key: string): Promise<any> {
    return memory[key]
  }

  async function delete(key: string): Promise<void> {
    delete memory[key]
  }

  return {
    set,
    get,
    delete
  }
}
```

## Adaptador de armazenamento Redis

Aqui está um adaptador Redis pronto para produção que demonstra o gerenciamento de ciclo de vida.

### Implementação: `component.ts`

```tsx
import { createClient, RedisClientType } from 'redis'
import { IStorage } from '../../types'
import { RedisConnectionError } from './errors'

export function createRedisStorageAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IStorage {
  const { config, logs } = components
  const logger = logs.getLogger('redis-storage')
  
  const hostUrl = config.requireString('REDIS_URL')
  const client: RedisClientType = createClient({ url: hostUrl })

  // Método de ciclo de vida: chamado quando o componente inicia
  async function start(): Promise<void> {
    try {
      await client.connect()
      logger.info('Cliente Redis conectado com sucesso')
    } catch (error) {
      logger.error('Falha ao conectar ao Redis', error)
      throw new RedisConnectionError('Não foi possível conectar ao Redis')
    }
  }

  // Método de ciclo de vida: chamado quando o componente para
  async function stop(): Promise<void> {
    try {
      await client.quit()
      logger.info('Cliente Redis desconectado')
    } catch (error) {
      logger.error('Erro ao desconectar o cliente Redis', error)
    }
  }

  async function set(key: string, value: any): Promise<void> {
    const serialized = JSON.stringify(value)
    await client.set(key, serialized)
    logger.debug(`Chave definida: ${key}`)
  }

  async function get(key: string): Promise<any> {
    const value = await client.get(key)
    logger.debug(`Chave obtida: ${key}`)
    return value ? JSON.parse(value) : null
  }

  async function delete(key: string): Promise<void> {
    await client.del(key)
    logger.debug(`Chave excluída: ${key}`)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    set,
    get,
    delete
  }
}
```

### Tratamento de erros: `errors.ts`

```tsx
export class RedisConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'RedisConnectionError'
  }
}
```

## Exemplo de adaptador de banco de dados

Aqui está um exemplo de um adaptador de banco de dados PostgreSQL.

```tsx
import { Pool, QueryResult } from 'pg'
import { IDatabase } from '../../types'

export function createPostgresAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IDatabase {
  const { config, logs } = components
  const logger = logs.getLogger('postgres-adapter')

  const pool = new Pool({
    connectionString: config.requireString('DATABASE_URL'),
    max: config.getNumber('DATABASE_POOL_SIZE', 20)
  })

  async function start(): Promise<void> {
    // Testar a conexão
    const client = await pool.connect()
    logger.info('A conexão com o banco de dados foi inicializada com sucesso')
    client.release()
  }

  async function stop(): Promise<void> {
    await pool.end()
    logger.info('A conexão com o banco de dados foi encerrada')
  }

  async function query<T = any>(
    sql: string,
    params?: any[]
  ): Promise<QueryResult<T>> {
    logger.debug('Executando consulta', { sql, params })
    return pool.query<T>(sql, params)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    consulta
  }
}
```

## Métodos de ciclo de vida

WKC fornece métodos especiais de ciclo de vida que são chamados automaticamente:

* `[Lifecycle.ComponentStarted]` ou `[START_COMPONENT]` - Chamado quando o serviço inicia
* `[Lifecycle.ComponentStopped]` ou `[STOP_COMPONENT]` - Chamado quando o serviço é encerrado

### Quando usar métodos de ciclo de vida

Use métodos de ciclo de vida quando seu adaptador precisar:

* Estabelecer conexões (banco de dados, cache, fila de mensagens)
* Inicializar pools ou clientes
* Realizar verificações de saúde
* Limpar recursos ao encerrar
* Fechar conexões com elegância

## Melhores práticas

### 1. Tratamento de erros

Sempre trate erros de conexão e lance erros personalizados significativos:

```tsx
try {
  await client.connect()
} catch (error) {
  logger.error('Falha na conexão', error)
  throw new ConnectionError('Falha ao conectar ao serviço externo')
}
```

### 2. Configuração

Use o componente de config para gerenciar as configurações do adapter:

```tsx
const {
  host: config.requireString('DB_HOST'),
  port: config.getNumber('DB_PORT', 5432),
  timeout: config.getNumber('DB_TIMEOUT', 30000)
}
```

### 3. Registro

Registre operações importantes para depuração e monitoramento:

```tsx
logger.info('Operação concluída', { recordId, duration })
logger.warn('Consulta lenta detectada', { query, duration })
logger.error('Operação falhou', { error, context })
```

### 4. Segurança de tipos

Sempre tipifique corretamente seus valores de retorno:

```tsx
async function getUser(id: string): Promise<User | null> {
  const result = await query<User>('SELECT * FROM users WHERE id = $1', [id])
  return result.rows[0] || null
}
```

### 5. Gerenciamento de recursos

Sempre limpe os recursos no método de ciclo de vida stop:

```tsx
async function stop(): Promise<void> {
  await pool.end()
  await client.disconnect()
  logger.info('Recursos liberados')
}
```

## Testando adaptadores

Veja a [documentação de Testing Services (WKC)](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc/) para orientações detalhadas sobre como testar adapters.

### Exemplo rápido

```tsx
describe('when creating a memory storage adapter', () => {
  let storage: IStorage

  beforeEach(() => {
    storage = createMemoryStorageAdapter()
  })

  describe('when setting a value', () => {
    it('should store and retrieve the value', async () => {
      await storage.set('key', 'value')
      const result = await storage.get('key')
      expect(result).toBe('value')
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
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/well-known-components/adapters.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
