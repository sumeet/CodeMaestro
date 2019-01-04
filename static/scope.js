function CS_EVAL__(js_code, locals) {
    eval_string = Object.keys(locals).map(key => `var ${key} = locals.${key};`).join("\n");
    eval_string += "\n\n" + js_code;
    return eval(eval_string);
}