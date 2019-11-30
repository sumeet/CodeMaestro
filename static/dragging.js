// this makes windows draggable and resizable
function setupInteract(el, onWindowChange) {
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

       onWindowChange(event.deltaRect.left, event.deltaRect.top, event.rect.width, event.rect.height);
     });
}

// taken from the interactjs.io website
function dragMoveListener(event, onWindowChange) {
  var target = event.target;

  var x = parseFloat(target.style.left);
  var y = parseFloat(target.style.top);

  console.log("dragMoveListener dx " + event.dx.toString());
  console.log("dragMoveListener dy " + event.dy.toString());

//  target.style.left = `${x + event.dx}px`;
//  target.style.top = `${y + event.dy}px`;

  onWindowChange(event.dx, event.dy, null, null);
}

//function onWindowChange(onChangeFunc, posDx, posDy, newWidth, newHeight) {
//    onChangeFunc(posDx, posDy, newWidth, newHeight);
//}