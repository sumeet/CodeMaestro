// this makes windows draggable and resizable
document.addEventListener("DOMContentLoaded", function(event) {
   console.log('initializing dragging');
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
       target.style.height = event.rect.height + 'px';

       // translate when resizing from top or left edges
       var x = parseFloat(target.style.left);
       var y = parseFloat(target.style.top);

       target.style.left = `${x + event.deltaRect.left}px`;
       target.style.top = `${y + event.deltaRect.top}px`;
     });
});


// taken from the interactjs.io website
function dragMoveListener(event) {
  var target = event.target;

  var x = parseFloat(target.style.left);
  var y = parseFloat(target.style.top);

  target.style.left = `${x + event.dx}px`;
  target.style.top = `${y + event.dy}px`;
}