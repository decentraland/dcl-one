function snapshot(body) {
    const url = body.url;
    const init = { ...body.init };
    if (init.method === null) init.method = undefined;
    if (init.body === null) init.body = undefined;
    if (typeof url !== "string" || (init.method !== undefined && typeof init.method !== "string") ||
        (init.body !== undefined && typeof init.body !== "string")) {
        throw new TypeError("signed fetch URL, method and body must be strings");
    }
    const headers = [];
    const add = (name, value) => {
        if (typeof name !== "string" || typeof value !== "string") throw new TypeError("signed fetch headers must be strings");
        const lower = name.toLowerCase();
        if (!lower.startsWith("x-identity-") && lower !== "x-authoritative-scope") {
            headers.push([name, value]);
        }
    };
    const input = init.headers;
    if (Array.isArray(input)) {
        for (const [name, value] of input) add(name, value);
    } else if (input && typeof input.forEach === "function") {
        input.forEach((value, name) => add(name, value));
    } else if (input) {
        for (const [name, value] of Object.entries(input)) add(name, value);
    }
    init.headers = headers;
    return { url, init };
}

module.exports.signedFetch = async function (body) {
    const { url, init } = snapshot(body);
    const headers = await Deno.core.ops.op_signed_fetch_headers(url, init.method, init.body);
    for (const [name, value] of headers) init.headers.push([name, value]);
    const response = await fetch(url, init);
    return {
        ok: response.ok,
        status: response.status,
        statusText: response.statusText,
        headers: response.headers,
        body: await response.text(),
    };
};

module.exports.getHeaders = async function (body) {
    const { url, init } = snapshot(body);
    const result = await Deno.core.ops.op_signed_fetch_headers(url, init.method, init.body);
    const headers = {};
    for (const [name, value] of result) headers[name] = value;
    return { headers };
};
