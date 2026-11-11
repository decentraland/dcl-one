> Conecte uma campanha de Rewards a uma scene do Decentraland ou a um servidor externo para conceder wearables e emotes automaticamente.

# Integrações

Depois de criar e configurar uma campanha e garantir que ela tenha estoque suficiente para fornecer recompensas, o próximo passo é conectar a campanha a um gatilho de recompensas. Esse gatilho pode ser uma Scene, uma Quest ou um servidor externo. Esta seção explica como diferentes integrações com Rewards podem ser feitas.

## Conceder recompensas a partir de uma scene

Rewards pode ser integrado diretamente em scenes do Decentraland, mas essa abordagem traz alguns riscos. Como a lógica está incorporada ao código da scene que os usuários podem acessar, não é recomendado para mintar itens com raridade inferior a \[EPIC]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#rarity).

Tenha em mente que usuários determinados com conhecimento técnico suficiente poderiam potencialmente contornar medidas de segurança como captchas, alterar seus endereços IP e mintar todos os itens disponíveis, que então poderiam vender no Marketplace. A principal proteção contra isso é garantir um fornecimento suficiente de itens, para que todos tenham uma chance justa de receber uma recompensa.

### Flags recomendadas do dispenser

As seguintes configurações do dispenser são recomendadas para reduzir o risco de exploits neste cenário:

* \[Limitar atribuições]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#limit-assignments)
* \[Assinatura do beneficiário]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#beneficiary-signature)
* \[Proteção de captcha]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#captcha-protection)
* \[Conectado ao Decentraland]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#connected-to-decentraland)
* \[Posição dentro de Decentraland]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#position-inside-decentraland) (se isso se aplicar ao seu caso de uso)

### Exemplo

```tsx
import { getPlayer } from '@dcl/sdk/src/players'
import { signedFetch } from '@decentraland/SignedFetch'
import { getRealm } from '~system/Runtime'

export function main() {
  // 1. Obter o desafio de captcha para mostrar ao usuário
  const request = await fetch(`https://rewards.decentraland.org/api/captcha`, {
    method: 'POST',
  })
  const captcha = await request.json()

  // 2. Exibir o captcha para o jogador concluir - Veja um exemplo em studios.decentraland.org/resources

  // 3. Obter dados do usuário
  const user = getPlayer()

  // 4. Obter o realm atual
  const realmInfo = await getRealm({})

  // 5. Enviar solicitação para atribuir um wearable/emote
  const assignRequest = await signedFetch('https://rewards.decentraland.org/api/rewards', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      campaign_key: '[DISPENSER_KEY]', // chave do dispenser
      beneficiary: user.userId, // endereço Ethereum
      catalyst: realmInfo.baseUrl, // domínio do catalyst
      captcha_id: captcha.data.id, // "9e6b2d07-b47b-4204-ae87-9c4dea48f9b7"
      captcha_value: '[CAPTCHA_VALUE]', // "123456"
    }),
  })

  const reward = await assignRequest.json()
```

## Conceder recompensas a partir de um Decentraland Quests

Esta integração legada aponta para a documentação arquivada do Quests.

Você pode integrar facilmente Rewards com o [Decentraland Quests](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/overview.md), isso é ideal se você quiser recompensar os usuários por concluir uma quest.

### Flags recomendadas do dispenser

As seguintes configurações do dispenser são recomendadas para reduzir o risco de exploits neste cenário:

* \[Limitar atribuições]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#limit-assignments) (se isso se aplicar ao seu caso de uso)

Qualquer uma das outras flags fará sua integração falhar; evite usá-las.


> [!WARNING]
> ⚠️ A chave do dispenser deve ser mantida em segredo, portanto você nunca deve expô-la ao usuário em nenhum momento.


### Exemplo

Para integrar sua Quest com o serviço Rewards, você só precisa de uma chave do dispenser e de [configurar um webhook](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/rewards.md) para conceder recompensas.

```js
{
    // ...
    "reward": {
        "hook": {
            "webhookUrl": "https://rewars.decentraland.org/api/rewards",
            "requestBody": {
                "campaign_key": "[DISPENSER_KEY]",
                "beneficiary": "{user_address}"
            }
        },
        // ...
    }
}
```

## Conceder recompensas a partir de um servidor personalizado

Você pode integrar Rewards diretamente do seu servidor, o que é ideal para realizar verificações extras antes de mintar itens. Uma vantagem adicional é que, ao contrário do código da scene, o código do seu servidor pode não ser público, tornando mais difícil para os usuários descobrirem e explorarem vulnerabilidades.

### Flags recomendadas do dispenser

As seguintes configurações do dispenser são recomendadas para reduzir o risco de exploits neste cenário:

* \[Limitar atribuições]\([Ver documentação](/docs/creator/content-creator-pt/readme/)#limit-assignments) (se isso se aplicar ao seu caso de uso)

Ativar qualquer uma das outras flags pode complicar sua integração ou, dependendo do seu caso de uso, potencialmente fazê-la falhar. Portanto, não é recomendado usá-las, a menos que haja uma necessidade específica. No entanto, você pode querer explorar seus possíveis benefícios.


> [!WARNING]
> ⚠️ A chave do dispenser deve ser mantida em segredo, portanto você nunca deve expô-la ao usuário em nenhum momento.


### Exemplo

```tsx
const request = await fetch('https://rewards.decentraland.org/api/rewards', {
	method: 'POST',
	headers: {
		'Content-Type': 'application/json',
	},
	body: JSON.stringify({
		campaign_key: '[DISPENSER_KEY]',
		beneficiary: '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', // endereço Ethereum
	}),
})

const response = await request.json()
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/recompensas/integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
