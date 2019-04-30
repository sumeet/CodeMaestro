clickHandlers = []

function showRightClickMenu(elId, event) {
    var el = document.getElementById(elId);
    if (el) {
        el.style.left = `${event.pageX}px`;
        el.style.top = `${event.pageY}px`;
        el.style.display = "";
        console.log(el.style.left);
        console.log(el.style.right);
    }
}

document.addEventListener("click", function() {
    var els = document.getElementsByClassName("context_menu");
    for (let el of els) {
        el.style.display = "none";
    }
});

