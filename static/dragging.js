WINDOW_ONCHANGE_HANDLER_BY_ELEMENT_ID = {};

// this makes windows draggable and resizable
document.addEventListener("DOMContentLoaded", function(event) {
   interact('.window').draggable({
     allowFrom: '.window-title',
     onmove: dragMoveListener,
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

       // update the element's width
       target.style.width  = event.rect.width + 'px';
       // ULTRA HAXXX: sometimes we get events telling us to resize height
       // to something really small right after a user-triggered resize, and
       // we should just ignore those
       if (event.rect.height > 100) {
           target.style.height = event.rect.height + 'px';
       }

       // translate when resizing from top or left edges
       var x = parseFloat(target.style.left);
       var y = parseFloat(target.style.top);

       target.style.left = `${x + event.deltaRect.left}px`;
       target.style.top = `${y + event.deltaRect.top}px`;

       // there's a weird bug here, sometimes the height is sent to us as 0. not sure what's causing that, but let's just
       // not do anything if we find that
       if (event.rect.width === 0 || event.rect.height === 0) {
           return;
       }

       onWindowChange(target.id, event.deltaRect.left, event.deltaRect.top, event.rect.width, event.rect.height);
     });
});

// taken from the interactjs.io website
function dragMoveListener(event) {
  var target = event.target;

  var x = parseFloat(target.style.left);
  var y = parseFloat(target.style.top);

  target.style.left = `${x + event.dx}px`;
  target.style.top = `${y + event.dy}px`;

  onWindowChange(target.id, event.dx, event.dy, null, null);
}

// newWidth and newHeight may be null if there's no change (if the window was dragged, but not resized)
function onWindowChange(id, posDx, posDy, newWidth, newHeight) {
    const onChangeFunc = WINDOW_ONCHANGE_HANDLER_BY_ELEMENT_ID[id];
    if (!onChangeFunc) {
        console.log("couldn't find onChangeFunc for " + id.toString());
        return;
    }
    onChangeFunc(posDx, posDy, newWidth, newHeight);
}