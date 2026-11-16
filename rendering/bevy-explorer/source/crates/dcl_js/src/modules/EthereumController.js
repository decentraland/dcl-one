const allowMethodlist = [
    'eth_sendTransaction',
    'eth_getTransactionReceipt',
    'eth_estimateGas',
    'eth_call',
    'eth_getBalance',
    'eth_getStorageAt',
    'eth_blockNumber',
    'eth_gasPrice',
    'eth_protocolVersion',
    'net_version',
    'web3_sha3',
    'web3_clientVersion',
    'eth_getTransactionCount',
    'eth_getBlockByNumber',
    'eth_requestAccounts',
    'eth_signTypedData_v4',
    'eth_getCode'
]

module.exports.sendAsync = async function (message) {
    if (
        typeof message !== 'object' ||
        typeof message.id !== 'number' ||
        typeof message.method !== 'string' ||
        typeof message.jsonParams !== 'string'
    ) {
        throw new Error('Invalid JSON-RPC message')
    }

    if (!allowMethodlist.includes(message.method)) {
        throw new Error(`The Ethereum method "${message.method}" is not allowed on Decentraland Provider`)
    }

    const resValue = await Deno.core.ops.op_send_async(message.method, message.jsonParams)

    const result = {
        id: message.id,
        jsonrpc: "2.0",
        result: resValue
    }

    return {
        jsonAnyResponse: JSON.stringify(result)
    }
}

// These legacy SDK6 methods retain the protobuf response envelopes.
module.exports.requirePayment = async function (_body) {
    // Legacy payment requests do not submit transactions.
    return { jsonAnyResponse: '{}' }
}

module.exports.signMessage = async function (body) {
    const fields = Object.entries(body.message)
        .map(([key, value]) => `${key}: ${value}`).join('\n')
    const message = '# DCL Signed message\n' + fields
    const hexEncodedMessage = '0x' + Array.from(new TextEncoder().encode(message),
        byte => byte.toString(16).padStart(2, '0')).join('')
    let signature = ''
    try {
        const result = await Deno.core.ops.op_sign_legacy_message(fields)
        signature = typeof result === 'string' ? result : ''
    } catch (_error) {
        // Rejection and unavailable wallets use the legacy empty-signature response.
    }
    return { message, hexEncodedMessage, signature }
}

module.exports.convertMessageToObject = async function (body) {
    let message = body.message
    if (message.indexOf('# DCL Signed message') === 0) message = message.slice(21)
    const dict = {}
    for (const line of message.split('\n')) {
        const [key, value] = line.split(':')
        // Define own map entries, including keys such as __proto__, without changing prototypes.
        Object.defineProperty(dict, key, {
            value: value.trim(), enumerable: true, configurable: true, writable: true
        })
    }
    return { dict }
}

module.exports.getUserAccount = async function (_body) {
    const address = await Deno.core.ops.op_get_user_account()
    return address == null ? {} : { address }
}
