function toggleMode() {
    var body = document.body;

    // Toggle classes for dark and compact mode
    body.classList.toggle('dark');

    // Store the modes in localStorage for persistence across page reloads
    localStorage.setItem('darkMode', body.classList.contains('dark'));
}

// On page load, check localStorage and apply the saved modes if any
window.onload = function() {
    var body = document.body;

    // Apply saved dark mode
    if (localStorage.getItem('darkMode') === 'true') {
        body.classList.add('dark');
    }
};