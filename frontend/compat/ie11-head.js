/* External and synchronous: select the IE layout before the first stylesheet paints. */
(function () {
  if (!document.documentMode) return;
  var root = document.documentElement;
  if (!/(^|\s)legacy-ie(\s|$)/.test(root.className)) root.className += ' legacy-ie';
  root.setAttribute('data-production-document-mode', String(document.documentMode));
}());
