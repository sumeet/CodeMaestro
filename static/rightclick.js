function showRightClickMenu(el, pageX, pageY) {
    // haxx, have to account for this because of body { zoom: } in main.css
    const globalZoom = 110;
    if (el) {
        el.style.left = `${pageX * (100 / globalZoom)}px`;
        el.style.top = `${pageY * (100 / globalZoom)}px`;
        el.style.display = "";
    }
}

// hide context menus after clicking on anything
document.addEventListener("click", function() {
    var els = document.getElementsByClassName("context_menu");
    for (let el of els) {
        el.style.display = "none";
    }
}, true);
