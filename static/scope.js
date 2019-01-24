function CS_EVAL__(js_code, locals) {
    eval_string = Object.keys(locals).map(key => `var ${key} = locals.${key};`).join("\n");
    eval_string += "\n\n" + js_code;
    return eval(eval_string);
}

async function CS_FETCH__(url) {
    var resp = await fetch(url);
    return {
        text: await resp.text(),
        status: resp.status,
        headers: resp.headers,
    };
}
