// called back with data from inside local file
// from https://gist.github.com/liabru/11263124
function openFileDialog(callback) {
    var element = document.createElement('div');
    element.innerHTML = '<input type="file">';
    var fileInput = element.firstChild;

    fileInput.addEventListener('change', function() {
        var file = fileInput.files[0];
        var reader = new FileReader();

        // TODO: remove this after testing
        reader.onload = function() {
            console.log(reader.result);
        };

        callback(reader.readAsText(file));
    });
    fileInput.click();
}

// opens up a save file dialog in the browser, prompting the user to save the file
// from https://stackoverflow.com/a/30832210
function saveFile(contents, filename, mimetype) {
}