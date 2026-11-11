# Cadeia de autenticação

Muitas ações no protocolo Decentraland exigem ou se beneficiam de autorização com uma assinatura da conta Ethereum do usuário. Por exemplo:

1. Enviar uma nova versão de qualquer [Entity](/docs/contributor/contributor-pt/conteudo/entities/) que eles possuem (como o perfil deles).
2. Autenticar-se em serviços de terceiros.
3. Autorizar delegados a agir em seu nome.

Para realizar essas ações, usuários e delegados devem assinar payloads que descrevem sua intenção e produzir uma *cadeia de autenticação*.

## Introdução

As cadeias de autenticação encapsulam uma série de etapas de verificação, com cada etapa dependendo da verificação bem-sucedida da anterior. Todas as etapas devem ser consideradas válidas pelo verificador para que qualquer ação restrita possa ocorrer.

Toda cadeia começa identificando o usuário e termina com um payload assinado representando a ação solicitada. Assim, a menor cadeia contém dois elementos, que indicam:

```md
1. A autoridade é a conta Ethereum <user-address>
2. O payload é <payload>, autorizado por <user-signature>
```

Essa cadeia básica pode ser avaliada verificando se a chave pública da assinatura corresponde ao endereço. Nesse caso, é equivalente a uma assinatura simples.

Quando os usuários autorizam delegados a agir em seu nome, etapas intermediárias aparecem na cadeia. Por exemplo, uma cadeia com uma única delegação indicaria:

```md
1. A autoridade é a conta Ethereum <user-address>
2. O delegado é <delegate-address>, autorizado por <user-signature> até <date>
3. O payload é <payload>, autorizado por <delegate-signature>
```

As cadeias são mais longas quando os delegados autorizam seus próprios delegados. Em outras palavras, a autorização é transitiva.


> [!NOTE]
> Você pode pensar nas cadeias de autenticação como análogas às cadeias de certificados TLS usadas em HTTPS, com o usuário como equivalente da autoridade raiz.


Essa cadeia de delegado único é a forma mais comum de autorização usada em Decentraland, já que os usuários autorizam uma chave para seu World Explorer para evitar ter de assinar cada ação individual com sua conta Ethereum.

### Construindo uma Cadeia <a href="#constructing" id="constructing"></a>

Cada etapa na cadeia de autenticação contém três informações: um `tipo`, um `payload` e uma `assinatura`.

| Campo        | Valor                                                                             |
| ------------ | --------------------------------------------------------------------------------- |
| `tipo`       | O nome de um tipo ([veja abaixo](#types)).                                        |
| `payload`    | ). Uma string dependente do tipo.                                                 |
| `assinatura` | A assinatura Ethereum codificada em hexadecimal de `payload`, começando com `0x`. |


> [!NOTE]
> Como a serialização mais comum de uma cadeia de autenticação é um array JSON, os exemplos abaixo são apresentados nessa forma. Veja [Transmitindo uma Cadeia](#transmitting) para mais detalhes.


### Primeira Etapa: Identificação

A primeira etapa, que identifica a autoridade original (ou seja, o usuário), deve atender a estas condições:

1. O `tipo` é [`SIGNER`](#signer)
2. O `payload` é a conta Ethereum codificada.
3. O `assinatura` está vazio.

Por exemplo:

```json
// Primeira etapa em qualquer cadeia de autenticação:
{
  "type": "SIGNER",
  "payload": "0xdB055877e6c13b6A6B25aBcAA29B393777dD0a73",
  "signature": ""
}
```

A segunda etapa deve carregar uma `assinatura` dessa conta para sua `payload`.

### Etapas Intermediárias: Delegação

Quando delegados agem em nome do usuário, um item é adicionado ao meio da cadeia para cada um deles. As condições para essas etapas são:

1. O tipo é [`ECDSA_EPHEMERAL`](#ecdsa_ephemeral)
2. O `payload` é um texto especialmente criado (veja abaixo).

O `payload` foi projetado para ser fácil de ler e fácil de analisar, já que tanto humanos (ao usar a UI da wallet) quanto programas (ao criar e validar) devem trabalhar com ele. Ele contém exatamente 3 linhas de texto simples com distinção entre maiúsculas e minúsculas:

```md
<purpose>
Endereço efêmero: <delegate-address>
Expiração: <date>
```

Por exemplo, este é um payload típico usado pelos World Explorers durante o login, quando geram sua chave delegada temporária para o usuário aprovar:

```
Login do Decentraland
Endereço efêmero: 0xBB9aDF0183b1742196A4Aa55622D5838f4f483a7
Expiração: 2023-02-25T13:00:19.730Z
```

Observe que:

1. O `Endereço efêmero` não precisa ser uma conta Ethereum real com fundos na blockchain. O campo pode representar apenas uma chave pública temporária.
2. O `Expiração` é uma data e hora serializadas em formato [ISO-8601](https://en.wikipedia.org/wiki/ISO_8601) . Não precisa ser UTC.
3. A chave do delegado deve ser renovada periodicamente com uma nova assinatura do usuário, devido a essa data de expiração.


> [!NOTE]
> É prática padrão (e altamente recomendada) gerar uma nova chave para cada renovação (daí o nome `ECDSA_EPHEMERAL`).


Um exemplo de etapa de delegação (valores abreviados para maior clareza):

```json
{
  "type": "ECDSA_EPHEMERAL",
  "payload": "Decentraland Login\nEndereço efêmero: 0xBBa7...\nExpiração: 2021-01-25T...",
  "signature": "0x1370a4120a7cb0d2f6e4a5..."
}
```

A próxima etapa, seja outra delegação ou a autorização final, carrega uma assinatura da chave deste delegado.

### Última Etapa: Autorização

Após o `SIGNER` ter sido especificado e todos os `ECDSA_EPHEMERAL` as chaves de delegado terem sido validadas por meio da verificação da cadeia de assinaturas intermediárias, a etapa final é a ação real que precisa ser autorizada.

O `payload` depende do `tipo` da etapa. Por exemplo, se um usuário estiver enviando um novo perfil (ou qualquer Entity que possua) para um content server, o último elemento terá esta forma:

```json
{
  // O tipo indica que `payload` é um ID de Entity (detalhes na seção Content):
  "type": "ECDSA_SIGNED_ENTITY",

  // O payload é a string bruta do ID:
  "payload": "bafkreicfbg7ybpuoslkcf6x2vfnvzl5vwgqtb2pnheqiut2i4sgpblicqi",

  // A assinatura é produzida pela conta da etapa anterior (usuário ou delegado):
  "signature": "0x7e71dbbab..."
}
```

Se essa última assinatura também for válida, a ação pode prosseguir.

### Transmitindo uma Cadeia <a href="#transmitting" id="transmitting"></a>

Como mencionado acima, a serialização mais comum de uma cadeia de autenticação é um array JSON. Essa é a abordagem recomendada.

No entanto, o protocolo não impõe isso. Os desenvolvedores têm permissão para usar uma estratégia de serialização alternativa se ela for mais conveniente para um caso de uso específico, como YAML, arquivos CSV, formatos binários otimizados ou strings simples com campos delimitados.

Um exemplo de serialização alternativa dentro do próprio protocolo pode ser encontrado no [`SignedFetch`](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/signed-fetch/) módulo, que usa uma sequência de cabeçalhos HTTP em vez de um array JSON.

### Escolhendo uma Data de Expiração

Ao selecionar a duração válida para uma chave de delegado, há uma compensação: expirações mais curtas aumentam a segurança, mas expirações mais longas melhoram a experiência do usuário (já que os delegados precisam ser renovados com interação humana com menor frequência).

Não existe uma estratégia universal para decidir qual deve ser a janela de tempo válida. O World Explorer da Fundação, como referência, solicita autorização para sua chave de delegado por um mês.


> [!NOTE]
> Chaves efêmeras nunca devem manter fundos nem possuir a capacidade de transferir ativos digitais. É muito mais seguro para os usuários finais se o vazamento de uma chave efêmera não puder resultar em perdas financeiras.


## Formalização

O que segue é uma definição mais formal e precisa dos processos envolvidos. Siga estas instruções para lidar com sucesso com cadeias de autenticação.

### Criação

Os clientes que montam uma cadeia de autenticação para o usuário seguem estas etapas:

1. Adicione a etapa de identificação:
   1. Defina o `tipo` para `SIGNER`.
   2. Defina o `payload` para o endereço Ethereum do usuário.
   3. Defina o `assinatura` para uma string vazia.
2. Adicione etapas de delegação:
   1. Gere ou use uma chave privada de delegado existente (pode exigir interação).
   2. Calcule o endereço Ethereum do delegado derivado da chave pública correspondente.
   3. Defina o `tipo` para `ECDSA_EPHEMERAL`.
   4. Defina o `expiração` para uma data no futuro.
   5. Escolha um `propósito` para esta chave.
   6. Defina o `payload` para esta forma exata:

      ```md
      <purpose>
      Endereço efêmero: <delegate-address>
      Expiração: <date>
      ```
   7. Defina o `assinatura` campo para a `payload` assinatura da chave anterior (usuário ou delegado).
   8. Repita para todos os delegados sucessivos.
3. Adicione a etapa de autorização da ação:
   1. Defina o `tipo` para um valor válido ([veja abaixo](#types))
   2. Defina o `payload` para o valor específico do tipo (como o ID da Entity).
   3. Defina o `assinatura` campo para a `payload` assinatura da chave anterior (usuário ou delegado).
4. Envie a cadeia de autenticação para o verificador.

### Verificação

Os content servers e serviços de terceiros que implementam a verificação seguem estas etapas:

1. Verifique a identificação:
   1. Verifique o `tipo` é `SIGNER`.
   2. Verifique o `payload` é o endereço Ethereum do usuário.
   3. Verifique o `assinatura` é uma string vazia.
2. Verifique os delegados:
   1. Verifique o `tipo` é `ECDSA_EPHEMERAL`.
   2. Verifique o `payload` está nesta forma e extraia os campos:

      ```md
      <purpose>
      Endereço efêmero: <delegate-address>
      Expiração: <date>
      ```
   3. Verifique o `data` ainda está no futuro.
   4. Verifique o `propósito` é suportado pelo seu serviço.
   5. Verifique o `assinatura` é válido para o dado `payload` e a chave pública anterior.
   6. Repita para todos os delegados sucessivos.
3. Verifique a autorização da ação:
   1. Verifique o `tipo` é um valor válido ([veja abaixo](#types)).
   2. Verifique o `payload` é válido para este `tipo`.
   3. Verifique o `assinatura` é válido para o dado `payload` e a chave pública anterior.
4. Aceite a cadeia de autenticação.

### Tipos de Ação e Propósitos Padrão <a href="#types" id="types"></a>

O `tipo` e `payload` valores de identificação e delegação são padrão e devem ser verificados conforme descrito acima, mas clientes e serviços podem concordar sobre qualquer ação `tipo`, e sua `payload` estrutura, assim como definir um `propósito` para suas chaves de delegado que considerem válido.

O protocolo define três tipos padrão e um propósito padrão:

#### Tipo `SIGNER` <a href="#signer" id="signer"></a>

**Deve** ser a etapa inicial `tipo` na cadeia, onde `payload` é o endereço Ethereum do usuário e `assinatura` é uma string vazia.

#### Tipo `ECDSA_EPHEMERAL` <a href="#ecdsa_ephemeral" id="ecdsa_ephemeral"></a>

**Deve** ser o `tipo` para etapas intermediárias na cadeia, onde `payload` está na forma descrita acima.

#### Tipo `ECDSA_SIGNED_ENTITY` <a href="#ecdsa_signed_entity" id="ecdsa_signed_entity"></a>

A etapa final `tipo` na cadeia para autorizar implantações de Entity, onde `payload` é o ID de uma Entity de propriedade do usuário.

#### Propósito `Login do Decentraland` <a href="#decentraland-login" id="decentraland-login"></a>

O habitual `propósito` para World Explorers, assinado tanto ao entrar no mundo quanto ao renovar sua chave de delegado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/autenticacao/authchain.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
