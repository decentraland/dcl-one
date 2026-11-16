module.exports.event = async function (body) {
    const params = JSON.parse(body.jsonPayload)
    switch (body.type) {
        case 'Runtime.consoleAPICalled': {
            const [event] = params
            console.log('', ...event.args.map(value => 'value' in value ? value.value : value.unserializableValue))
            break
        }
        case 'Runtime.exceptionThrown': {
            const [{ exceptionDetails }] = params
            if (exceptionDetails.exception) {
                console.error(exceptionDetails.text,
                    exceptionDetails.exception.value || exceptionDetails.exception.unserializableValue)
            } else {
                console.error(exceptionDetails.text)
            }
            break
        }
    }
    return {}
}
