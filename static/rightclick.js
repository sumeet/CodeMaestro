function showRightClickMenu(el, pageX, pageY) {
    // haxx, have to account for this because of body { zoom: } in main.css
    const globalZoom = 110;
    if (el) {
        el.style.left = `${pageX * (100 / globalZoom)}px`;
        el.style.top = `${pageY * (100 / globalZoom)}px`;
        el.style.display = "";
    }
    insertContextMenuIntoGlobalDiv(el);
}

// hide context menus after clicking on anything
document.addEventListener("click", function() {
    removeContextMenuDivIfPresent();
}, true);

const CONTEXT_MENU_ID = "context_menu";

function removeContextMenuDivIfPresent() {
    const contextMenuDiv = document.getElementById(CONTEXT_MENU_ID);
    if (contextMenuDiv) {
        contextMenuDiv.remove();
    }
}

function insertContextMenuIntoGlobalDiv(el) {
    removeContextMenuDivIfPresent();
    el.id = "context_menu"
    document.body.appendChild(el);
}
