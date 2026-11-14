/* Select the real IE11 interface even when an older server or changed UA serves modern HTML. */
(function () {
  if (!document.documentMode) return;
  var root = document.documentElement;
  if (!/(^|\s)legacy-ie(\s|$)/.test(root.className)) root.className += ' legacy-ie';
  if (!/\/legacy\/index\.html$/i.test(window.location.pathname)) {
    window.location.replace('/legacy/index.html' + window.location.search + window.location.hash);
  }
}());
