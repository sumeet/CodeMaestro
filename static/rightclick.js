function showRightClickMenu(contextMenuEl, contextMenuTriggerEl, pageX, pageY) {
    // haxx, have to account for this because of body { zoom: } in main.css
    const globalZoom = 110;
    if (contextMenuEl) {
        contextMenuEl.style.left = `${pageX * (100 / globalZoom)}px`;
        contextMenuEl.style.top = `${pageY * (100 / globalZoom)}px`;
        contextMenuEl.style.display = "";

        insertContextMenuIntoGlobalDiv(contextMenuEl);
    }

    if (contextMenuTriggerEl) {
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
