// mutation observer:
// if focused node: store parent of focused node.
// else: focus previously stored parent

__PREVIOUSLY_FOCUSED_FOCUSABLE = null;

//badboy from autoscroll.js
new MutationObserver(function() {
    if (document.activeElement && document.activeElement.tagName != "BODY") {
        var closestFocusable = findClosestFocusable(document.activeElement);
        if (closestFocusable !== null) {
            __PREVIOUSLY_FOCUSED_FOCUSABLE = closestFocusable;
        }
    } else {
        if (__PREVIOUSLY_FOCUSED_FOCUSABLE !==  null) {
            __PREVIOUSLY_FOCUSED_FOCUSABLE.focus();
        }
    }
}).observe(window.document.documentElement, {childList: true, subtree: true});

var findClosestFocusable = function(el) {
    return el.closest("[tabindex='0']");
};
