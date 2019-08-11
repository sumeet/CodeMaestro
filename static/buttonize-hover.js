var displayButtonizedHoverOverlayOn = function(el, bgcolor) {
  el.insertAdjacentHTML("afterend", `<div style="position: absolute; top: ${el.offsetTop}px; left: ${el.offsetLeft}px; height: ${el.offsetHeight}px; width: ${el.offsetWidth}px; background-color: ${bgcolor};" class="buttonized-hover-overlay">&nbsp;</div>`);
};

var removeOverlays = function(el) {
  console.log('removeOverlays');
  el.querySelectorAll('.buttonized-hover-overlay').forEach(function(overlay) {
    overlay.remove();
  });
};
