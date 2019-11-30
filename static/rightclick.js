clickHandlers = []

function showRightClickMenu(el, pageX, pageY) {
    if (el) {
        el.style.left = `${pageX}px`;
        el.style.top = `${pageY}px`;
        el.style.display = "";
    }
}

document.addEventListener("click", function() {
    var els = document.getElementsByClassName("context_menu");
    for (let el of els) {
        el.style.display = "none";
    }
}, true);

