> Conecta una campaña de Rewards a una escena de Decentraland o a un servidor externo para conceder wearables y emotes automáticamente.

# Integraciones

Después de crear y configurar una campaña y asegurarte de que tiene suficiente stock para proporcionar recompensas, el siguiente paso es conectar la campaña a un disparador de recompensas. Este disparador puede ser una Scene, una Quest o un servidor externo. Esta sección explica cómo se pueden hacer diferentes integraciones con Rewards.

## Otorgar recompensas desde una escena

Rewards se puede integrar directamente en las escenas de Decentraland, pero este enfoque conlleva algunos riesgos. Como la lógica está incrustada en el código de la escena al que los usuarios pueden acceder, no se recomienda para mintear ítems con una rareza inferior a \[EPIC]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#rarity).

Ten en cuenta que los usuarios determinados con suficiente conocimiento técnico podrían eludir medidas de seguridad como los captchas, cambiar sus direcciones IP y mintear todos los ítems disponibles, que luego podrían vender en Marketplace. La principal medida de seguridad contra esto es garantizar un suministro suficiente de ítems, para que todos tengan una oportunidad justa de recibir una recompensa.

### Flags recomendados del dispenser

Se recomiendan las siguientes configuraciones del dispenser para reducir el riesgo de exploits en este escenario:

* \[Limit Assignments]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#limit-assignments)
* \[Beneficiary Signature]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#beneficiary-signature)
* \[Captcha Protection]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#captcha-protection)
* \[Connected to Decentraland]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#connected-to-decentraland)
* \[Position inside Decentraland]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)#position-inside-decentraland) (si aplica a tu caso de uso)

### Ejemplo

```tsx
import { getPlayer } from '@dcl/sdk/src/players'
import { signedFetch } from '@decentraland/SignedFetch'
import { getRealm } from '~system/Runtime'

export function main() {
  // 1. Obtener el desafío captcha para mostrar al usuario
  const request = await fetch(`https://rewards.decentraland.org/api/captcha`, {
    method: 'POST',
  })
  const captcha = await request.json()

  // 2. Mostrar el captcha para que el jugador lo complete - Consulta el ejemplo en studios.decentraland.org/resources

  // 3. Obtener datos del usuario
  const user = getPlayer()

  // 4. Obtener el realm actual
  const realmInfo = await getRealm({})

  // 5. Enviar la solicitud para asignar un wearable/emote
  const assignRequest = await signedFetch('https://rewards.decentraland.org/api/rewards', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      campaign_key: '[DISPENSER_KEY]', // clave del dispenser
      beneficiary: user.userId, // dirección de Ethereum
      catalyst: realmInfo.baseUrl, // dominio del catalyst
      captcha_id: captcha.data.id, // "9e6b2d07-b47b-4204-ae87-9c4dea48f9b7"
      captcha_value: '[CAPTCHA_VALUE]', // "123456"
    }),
  })

  const reward = await assignRequest.json()
```

## Otorgar recompensas desde Decentraland Quests

Esta integración heredada enlaza con la documentación archivada de Quests.

Puedes integrar fácilmente Rewards con [Decentraland Quests](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/overview.md), esto es ideal si quieres recompensar a los usuarios por completar una misión.

### Flags recomendados del dispenser

Se recomiendan las siguientes configuraciones del dispenser para reducir el riesgo de exploits en este escenario:

* \[Limit Assignments]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)(si aplica a tu caso de uso)

Cualquier otro de los demás flags hará que tu integración falle; evita usarlos.


> [!WARNING]
> ⚠️ La clave del dispenser debe mantenerse en secreto, así que nunca debes exponerla al usuario en ningún momento.


### Ejemplo

Para integrar tu Quest con el servicio Rewards, solo necesitas una clave de dispenser y [configurar un webhook](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/rewards.md) para otorgar recompensas.

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

## Otorgar recompensas desde un servidor personalizado

Puedes integrar Rewards directamente desde tu servidor, lo cual es ideal para realizar comprobaciones adicionales antes de mintear ítems. Una ventaja adicional es que, a diferencia del código de la escena, el código de tu servidor podría no ser público, lo que hace más difícil para los usuarios descubrir y explotar vulnerabilidades.

### Flags recomendados del dispenser

Se recomiendan las siguientes configuraciones del dispenser para reducir el riesgo de exploits en este escenario:

* \[Limit Assignments]\([Consulta la documentación](/docs/creator/content-creator-es/readme/)(si aplica a tu caso de uso)

Habilitar cualquiera de los otros flags podría complicar tu integración o, dependiendo de tu caso de uso, incluso hacer que falle. Por lo tanto, no se recomienda usarlos a menos que exista una necesidad específica. Sin embargo, quizá quieras explorar sus posibles beneficios.


> [!WARNING]
> ⚠️ La clave del dispenser debe mantenerse en secreto, así que nunca debes exponerla al usuario en ningún momento.


### Ejemplo

```tsx
const request = await fetch('https://rewards.decentraland.org/api/rewards', {
	method: 'POST',
	headers: {
		'Content-Type': 'application/json',
	},
	body: JSON.stringify({
		campaign_key: '[DISPENSER_KEY]',
		beneficiary: '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', // dirección de Ethereum
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
GET https://docs.decentraland.org/creator/content-creator-es/recompensas/integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
