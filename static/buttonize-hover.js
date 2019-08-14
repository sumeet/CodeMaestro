var displayButtonizedHoverOverlayOn = function(el, bgcolor) {
  el.parentElement.querySelectorAll('.buttonized-hover-overlay').forEach(function(overlay) {
    overlay.style.position = "absolute";
    overlay.style.top = `${el.offsetTop}px`;
    overlay.style.left = `${el.offsetLeft}px`;
    overlay.style.height = `${el.offsetHeight}px`;
    overlay.style.width = `${el.offsetWidth}px`;
    overlay.style.backgroundColor = bgcolor;
    overlay.style.display = "block";
  });
};

var removeOverlays = function(el) {
  el.querySelectorAll('.buttonized-hover-overlay').forEach(function(overlay) {
    overlay.style.height = "0px;"
    overlay.style.width = "0px;"
    overlay.style.display = "none";
  });
};
