clickHandlers = []

function showRightClickMenu(el, rightClickEvent) {
    if (el) {
        el.style.left = `${rightClickEvent.pageX}px`;
        el.style.top = `${rightClickEvent.pageY}px`;
        el.style.display = "";
    }
}

document.addEventListener("click", function() {
    var els = document.getElementsByClassName("context_menu");
    for (let el of els) {
        el.style.display = "none";
    }
}, true);

