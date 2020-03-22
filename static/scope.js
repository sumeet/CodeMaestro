function CS_EVAL__(js_code, locals) {
    eval_string = Object.keys(locals).map(key => `var ${key} = locals.${key};`).join("\n");
    eval_string += "\n\n" + js_code;
    return eval(eval_string);
}

// CS_FETCH__(@{request_url}, @{request_method}, @{request_headers}, @{request_body});

async function CS_FETCH__(url, method, headers, body) {
    // the fetch API doesn't support doing a GET request with a body specified.
    if (method == "GET") {
        body = undefined;
    }

    var resp = await fetch(url, {method, headers, body});
    return {
        text: await resp.text(),
        status: resp.status,
        headers: resp.headers,
    };
}
