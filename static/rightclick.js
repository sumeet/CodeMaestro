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

document.body.addEventListener("contextmenu", function(event) {
    event.preventDefault();
    event.stopPropagation();

    for (let triggerEl of allContextMenuTriggerEls()) {
        if (isWithin(trigger, event.pageX, event.pageY)) {
            console.log(triggerEl);
            var triggerFn = getRightClickListener(triggerEl);
            console.log(triggerFn);
            return triggerFn(cloneEvent(event));
        }
    }
}, true);

const RIGHT_CLICK_LISTENERS = new WeakMap();

function addRightClickListener(el, onEvent) {
    RIGHT_CLICK_LISTENERS.set(el, onEvent);
}

function getRightClickListener(el) {
    return RIGHT_CLICK_LISTENERS.get(el);
}

//from https://stackoverflow.com/a/20541207/149987
function cloneEvent(event) {
    return new event.constructor(event.type, event);
}

function allContextMenuTriggerEls() {
    return document.getElementsByClassName("context_menu_trigger");
}

//badboy from https://stackoverflow.com/a/28222246/149987
function isWithin(el, x, y) {
    var rect = el.getBoundingClientRect();
    var left = rect.left + window.scrollX;
    var right = rect.right + window.scrollX;
    var top = rect.top + window.scrollY;
    var bottom = rect.bottom + window.scrollY;
    var withinWidth = x >= left && x <= right;
    var withinHeight = y >= top && y <= bottom;
    return (withinWidth && withinHeight);
}
