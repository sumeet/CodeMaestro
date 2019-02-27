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
       var target = event.target,
         x = (parseFloat(target.getAttribute('data-x')) || 0),
         y = (parseFloat(target.getAttribute('data-y')) || 0);

       // update the element's style
       target.style.width  = event.rect.width + 'px';
       target.style.height = event.rect.height + 'px';

       // translate when resizing from top or left edges
       x += event.deltaRect.left;
       y += event.deltaRect.top;

       target.style.webkitTransform = target.style.transform =
           'translate(' + x + 'px,' + y + 'px)';

       target.setAttribute('data-x', x);
       target.setAttribute('data-y', y);
     });
});


// taken from the interactjs.io website
function dragMoveListener(event) {
  var target = event.target,
    // keep the dragged position in the data-x/data-y attributes
    x = (parseFloat(target.getAttribute('data-x')) || 0) + event.dx,
    y = (parseFloat(target.getAttribute('data-y')) || 0) + event.dy;

  // translate the element
  target.style.webkitTransform =
  target.style.transform =
    'translate(' + x + 'px, ' + y + 'px)';

  // update the posiion attributes
  target.setAttribute('data-x', x);
  target.setAttribute('data-y', y);
}