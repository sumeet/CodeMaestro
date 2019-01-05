function CS_EVAL__(js_code, locals) {
    eval_string = Object.keys(locals).map(key => `var ${key} = locals.${key};`).join("\n");
    eval_string += "\n\n" + js_code;
    return eval(eval_string);
}

async function CS_FETCH__(url) {
    console.log("making the damn request");
    var resp = await fetch(url);
    console.log("made the damn request!");
    return await resp.text();
}
