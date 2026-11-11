# Cadena de autenticación

Muchas acciones en el protocolo de Decentraland requieren o se benefician de una autorización con una firma de la cuenta Ethereum del usuario. Por ejemplo:

1. Subir una nueva versión de cualquier [Entity](/docs/contributor/contributor-es/contenido/entities/) que posean (como su perfil).
2. Autenticarse en servicios de terceros.
3. Autorizar a delegados para que actúen en su nombre.

Para realizar estas acciones, los usuarios y delegados deben firmar payloads que describan su intención y producir un *cadena de autenticación*.

## Introducción

Las cadenas de autenticación encapsulan una serie de pasos de verificación, en la que cada paso depende de la verificación exitosa del anterior. Todos los pasos deben ser considerados válidos por el verificador para que cualquier acción restringida pueda llevarse a cabo.

Toda cadena comienza identificando al usuario y termina con un payload firmado que representa la acción solicitada. La cadena más pequeña contiene, por tanto, dos elementos, que indican:

```md
1. La autoridad es la cuenta Ethereum <user-address>
2. El payload es <payload>, autorizado por <user-signature>
```

Esta cadena básica puede evaluarse verificando que la clave pública de la firma corresponde a la dirección. En este caso, equivale a una firma simple.

Cuando los usuarios autorizan a delegados para actuar en su nombre, aparecen pasos intermedios en la cadena. Por ejemplo, una cadena con una sola delegación indicaría:

```md
1. La autoridad es la cuenta Ethereum <user-address>
2. El delegado es <delegate-address>, autorizado por <user-signature> hasta <date>
3. El payload es <payload>, autorizado por <delegate-signature>
```

Las cadenas son más largas cuando los delegados autorizan a sus propios delegados. En otras palabras, la autorización es transitiva.


> [!NOTE]
> Se puede pensar en las cadenas de autenticación como análogas a las cadenas de certificados TLS usadas en HTTPS, con el usuario como equivalente de la autoridad raíz.


Esta cadena de un solo delegado es la forma de autorización más común usada en Decentraland, ya que los usuarios autorizan una clave para su World Explorer para evitar tener que firmar cada acción individual con su cuenta Ethereum.

### Construcción de una cadena <a href="#constructing" id="constructing"></a>

Cada paso en la cadena de autenticación contiene tres piezas de información: un `tipo`, una `carga útil` y una `firma`.

| Campo        | Valor                                                                                  |
| ------------ | -------------------------------------------------------------------------------------- |
| `tipo`       | El nombre de un tipo ([véase más abajo](#types)).                                      |
| `carga útil` | Una cadena dependiente del tipo.                                                       |
| `firma`      | La firma de Ethereum codificada en hexadecimal de `carga útil`, que comienza con `0x`. |


> [!NOTE]
> Dado que la serialización más común de una cadena de autenticación es un array JSON, los ejemplos siguientes se presentan en ese formato. Véase [Transmisión de una cadena](#transmitting) para más detalles.


### Primer paso: identificación

El primer paso, que identifica a la autoridad original (es decir, al usuario), debe cumplir estas condiciones:

1. El `tipo` es [`SIGNER`](#signer)
2. El `carga útil` es la cuenta Ethereum codificada.
3. El `firma` está vacío.

Por ejemplo:

```json
// Primer paso en cualquier cadena de autenticación:
{
  "type": "SIGNER",
  "payload": "0xdB055877e6c13b6A6B25aBcAA29B393777dD0a73",
  "signature": ""
}
```

El segundo paso debe llevar una `firma` de esta cuenta para su `carga útil`.

### Pasos intermedios: delegación

Cuando los delegados actúan en nombre del usuario, se añade un elemento en la parte media de la cadena por cada uno de ellos. Las condiciones de estos pasos son:

1. El tipo es [`ECDSA_EPHEMERAL`](#ecdsa_ephemeral)
2. El `carga útil` es un texto especialmente elaborado (véase abajo).

El `carga útil` está diseñado para ser fácil de leer y fácil de analizar, ya que tanto los humanos (cuando usan la UI de su wallet) como los programas (al elaborarlo y validarlo) deben trabajar con él. Contiene exactamente 3 líneas de texto plano sensible a mayúsculas y minúsculas:

```md
<purpose>
Dirección efímera: <delegate-address>
Caducidad: <date>
```

Por ejemplo, esta es una carga útil típica usada por World Explorers durante el inicio de sesión, cuando generan su clave temporal de delegado para que el usuario la apruebe:

```
Inicio de sesión de Decentraland
Dirección efímera: 0xBB9aDF0183b1742196A4Aa55622D5838f4f483a7
Caducidad: 2023-02-25T13:00:19.730Z
```

Ten en cuenta que:

1. El `Dirección efímera` no necesita ser una cuenta Ethereum real con fondos en la blockchain. El campo puede representar solo una clave pública temporal.
2. El `Caducidad` es una fecha y hora serializada en [ISO-8601](https://en.wikipedia.org/wiki/ISO_8601) formato. No necesita ser UTC.
3. La clave del delegado debe renovarse periódicamente con una firma nueva del usuario, debido a esta fecha de caducidad.


> [!NOTE]
> Es práctica estándar (y muy recomendable) generar una nueva clave para cada renovación (de ahí el nombre `ECDSA_EPHEMERAL`).


Un ejemplo de paso de delegación (valores abreviados para mayor claridad):

```json
{
  "type": "ECDSA_EPHEMERAL",
  "payload": "Inicio de sesión de Decentraland\nDirección efímera: 0xBBa7...\nCaducidad: 2021-01-25T...",
  "signature": "0x1370a4120a7cb0d2f6e4a5..."
}
```

El siguiente paso, ya sea otra delegación o la autorización final, lleva una firma de la clave de este delegado.

### Último paso: autorización

Después de que `SIGNER` se haya especificado y todas las `ECDSA_EPHEMERAL` las claves de delegado hayan sido validadas verificando la cadena de firmas intermedias, el paso final es la acción real que necesita ser autorizada.

El `carga útil` depende del `tipo` del paso. Por ejemplo, si un usuario está subiendo un nuevo perfil (o cualquier Entity que posea) a un servidor de contenido, el último elemento tendrá esta forma:

```json
{
  // El tipo indica que `payload` es un ID de Entity (detalles en la sección Content):
  "type": "ECDSA_SIGNED_ENTITY",

  // El payload es la cadena de ID en bruto:
  "payload": "bafkreicfbg7ybpuoslkcf6x2vfnvzl5vwgqtb2pnheqiut2i4sgpblicqi",

  // La firma es producida por la cuenta del paso anterior (usuario o delegado):
  "signature": "0x7e71dbbab..."
}
```

Si esta última firma también es válida, la acción puede continuar.

### Transmisión de una cadena <a href="#transmitting" id="transmitting"></a>

Como se mencionó antes, la serialización más común de una cadena de autenticación es un array JSON. Este es el enfoque recomendado.

Sin embargo, el protocolo no impone esto. Se permite a los desarrolladores usar una estrategia de serialización alternativa si resulta más conveniente para un caso de uso concreto, como YAML, archivos CSV, formatos binarios optimizados o cadenas simples con campos delimitados.

Un ejemplo de serialización alternativa dentro del propio protocolo puede encontrarse en el [`SignedFetch`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/signed-fetch/) módulo, que usa una secuencia de encabezados HTTP en lugar de un array JSON.

### Elegir una fecha de caducidad

Al seleccionar la duración válida para una clave de delegado, hay un equilibrio: caducidades más cortas aumentan la seguridad, pero caducidades más largas mejoran la experiencia del usuario (ya que los delegados tienen que renovarse con interacción humana con menor frecuencia).

No existe una estrategia universal para decidir cuál debe ser la ventana de tiempo válida. Como referencia, el World Explorer de la Foundation solicita autorización para su clave de delegado por un mes.


> [!NOTE]
> Las claves efímeras nunca deberían tener fondos ni poseer la capacidad de transferir activos digitales. Es mucho más seguro para los usuarios finales si la filtración de una clave efímera no puede traducirse en pérdidas financieras.


## Formalización

A continuación se presenta una definición más formal y precisa de los procesos implicados. Sigue estas instrucciones para manejar correctamente las cadenas de autenticación.

### Creación

Los clientes que elaboran una cadena de autenticación para el usuario siguen estos pasos:

1. Añade el paso de identificación:
   1. Establece el `tipo` en `SIGNER`.
   2. Establece el `carga útil` en la dirección Ethereum del usuario.
   3. Establece el `firma` en una cadena vacía.
2. Añade pasos de delegación:
   1. Genera o usa una clave privada de delegado existente (puede requerir interacción).
   2. Calcula la dirección Ethereum del delegado derivada de la clave pública correspondiente.
   3. Establece el `tipo` en `ECDSA_EPHEMERAL`.
   4. Establece el `caducidad` en una fecha futura.
   5. Elige un `propósito` para esta clave.
   6. Establece el `carga útil` a esta forma exacta:

      ```md
      <purpose>
      Dirección efímera: <delegate-address>
      Caducidad: <date>
      ```
   7. Establece el `firma` campo a la `carga útil` firma de la clave anterior (usuario o delegado).
   8. Repite para todos los delegados sucesivos.
3. Añade el paso de autorización de la acción:
   1. Establece el `tipo` a un valor válido ([véase más abajo](#types))
   2. Establece el `carga útil` al valor específico del tipo (como el ID de la Entity).
   3. Establece el `firma` campo a la `carga útil` firma de la clave anterior (usuario o delegado).
4. Envía la cadena de autenticación al verificador.

### Verificación

Los servidores de contenido y los servicios de terceros que implementan la verificación siguen estos pasos:

1. Verifica la identificación:
   1. Verifica que el `tipo` es `SIGNER`.
   2. Verifica que el `carga útil` sea la dirección Ethereum del usuario.
   3. Verifica que el `firma` sea una cadena vacía.
2. Verifica a los delegados:
   1. Verifica que el `tipo` es `ECDSA_EPHEMERAL`.
   2. Verifica que el `carga útil` esté en esta forma y extrae los campos:

      ```md
      <purpose>
      Dirección efímera: <delegate-address>
      Caducidad: <date>
      ```
   3. Verifica que el `fecha` sigue estando en el futuro.
   4. Verifica que el `propósito` es compatible con tu servicio.
   5. Verifica que el `firma` es válida para la `carga útil` y la clave pública anterior.
   6. Repite para todos los delegados sucesivos.
3. Verifica la autorización de la acción:
   1. Verifica que el `tipo` es un valor válido ([véase más abajo](#types)).
   2. Verifica que el `carga útil` es válido para este `tipo`.
   3. Verifica que el `firma` es válida para la `carga útil` y la clave pública anterior.
4. Acepta la cadena de autenticación.

### Tipos y propósitos de acción estándar <a href="#types" id="types"></a>

El `tipo` y `carga útil` Los valores para identificación y delegación son estándar y deben verificarse como se indicó arriba, pero los clientes y servicios pueden acordar cualquier acción `tipo`, y su `carga útil` estructura, así como establecer una `propósito` para sus claves de delegado que consideren válidas.

El protocolo define tres tipos estándar y un propósito estándar:

#### Tipo `SIGNER` <a href="#signer" id="signer"></a>

**Debe** ser el tipo inicial `tipo` en la cadena, donde `carga útil` es la dirección Ethereum del usuario y `firma` sea una cadena vacía.

#### Tipo `ECDSA_EPHEMERAL` <a href="#ecdsa_ephemeral" id="ecdsa_ephemeral"></a>

**Debe** ser el `tipo` para los pasos intermedios de la cadena, donde `carga útil` está en la forma descrita arriba.

#### Tipo `ECDSA_SIGNED_ENTITY` <a href="#ecdsa_signed_entity" id="ecdsa_signed_entity"></a>

El tipo final `tipo` en la cadena para autorizar despliegues de Entity, donde `carga útil` es el ID de una Entity propiedad del usuario.

#### Propósito `Inicio de sesión de Decentraland` <a href="#decentraland-login" id="decentraland-login"></a>

El uso habitual `propósito` para World Explorers, firmado tanto al iniciar sesión en el World como al renovar su clave de delegado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/autenticacion/authchain.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
