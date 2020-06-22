function showRightClickMenu(contextMenuEl, contextMenuTriggerEl, drawContextMenuActiveOverlay, pageX, pageY) {
    // haxx, have to account for this because of body { zoom: } in main.css
    const globalZoom = 110;

    // shift everything down to the right a bit, because that's how real right click menus work. they open close,
    // but not exactly where you right click, and you have to move the mouse to select the first menu item
    pageX += 5;
    pageY += 5;

    if (contextMenuEl) {
        contextMenuEl.style.left = `${pageX * (100 / globalZoom)}px`;
        contextMenuEl.style.top = `${pageY * (100 / globalZoom)}px`;
        contextMenuEl.style.display = "";

        insertContextMenuIntoGlobalDiv(contextMenuEl);
    }

    if (contextMenuTriggerEl && drawContextMenuActiveOverlay) {
        showContextMenuActiveOverlay(contextMenuTriggerEl);
    }
}

function showContextMenuActiveOverlay(contextMenuTriggerEl) {
    contextMenuTriggerEl.classList.add('triggered');
}

function hideContextMenuActiveOverlay(contextMenuTriggerEl) {
    contextMenuTriggerEl.classList.remove('triggered');
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

    document.querySelectorAll('.context_menu_trigger.triggered').forEach(hideContextMenuActiveOverlay);
}

function insertContextMenuIntoGlobalDiv(el) {
    removeContextMenuDivIfPresent();
    el.id = "context_menu"
    document.body.appendChild(el);
}
