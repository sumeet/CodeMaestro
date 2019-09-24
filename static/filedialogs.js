// called back with data from inside local file
// from https://gist.github.com/liabru/11263124
function openFileDialog(callback) {
    var element = document.createElement('div');
    element.innerHTML = '<input type="file">';
    var fileInput = element.firstChild;

    fileInput.addEventListener('change', function() {
        var file = fileInput.files[0];
        var reader = new FileReader();
        reader.onload = function() {
            callback(reader.result);
        };
        reader.readAsArrayBuffer(file);
    });
    fileInput.click();
}

// opens up a save file dialog in the browser, prompting the user to save the file
// from https://stackoverflow.com/a/30832210
function saveFile(bytes, filename, mimetype) {
    const arrayBuffer = new Uint8Array(bytes).buffer;
    var file = new Blob([arrayBuffer], {type: mimetype});

    var a = document.createElement("a"),
            url = URL.createObjectURL(file);
    a.download = filename;
    a.href = url;
    document.body.appendChild(a);
    a.click();
    setTimeout(function() {
        document.body.removeChild(a);
        window.URL.revokeObjectURL(url);
    }, 0);
}