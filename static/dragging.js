// this makes windows draggable and resizable
function setupInteract(el, onWindowChange) {
   // undo any previous interact hooks setup
   interact(el).unset();

   interact(el).draggable({
     allowFrom: '.window-title',
     onmove: function(event) { dragMoveListener(event, onWindowChange); },
   }).resizable({
    // resize from all edges and corners except the top. top is
    // reserved for moving (.drag-handle is on top)
    edges: {left: true, right: true, bottom: true, top: false},

    // keep the edges inside the parent
    restrictEdges: {
      outer: 'parent',
      endOnly: true,
    },
   })
     // taken from the interactjs.io website
     .on('resizemove', function (event) {
       var target = event.target;

       // there's a weird bug here, sometimes the height is sent to us as 0. not sure what's causing that, but let's just
       // not do anything if we find that
       if (event.rect.width === 0 || event.rect.height === 0) {
           return;
       }

       onWindowChange(target, event.deltaRect.left, event.deltaRect.top, event.rect.width, event.rect.height);
     });
}

// taken from the interactjs.io website
function dragMoveListener(event, onWindowChange) {
    onWindowChange(event.target, event.dx, event.dy, null, null);
}

////////////////////////////////////////////////////////////////////
/// the following is for drag+drop (too lazy to make dragdrop.js)
////////////////////////////////////////////////////////////////////

// craziness to get real BG color (see YewToolkit.drag_drop_source)
// from https://stackoverflow.com/a/41663840
function realBackgroundColor(elem) {
    var transparent = 'rgba(0, 0, 0, 0)';
    var transparentIE11 = 'transparent';
    if (!elem) return transparent;

    var bg = getComputedStyle(elem).backgroundColor;
    if (bg === transparent || bg === transparentIE11) {
        return realBackgroundColor(elem.parentElement);
    } else {
        return bg;
    }
}