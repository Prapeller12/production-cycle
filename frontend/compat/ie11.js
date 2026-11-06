/* DOM compatibility for the same application in the IE11/MSHTML engine. */
(function () {
  if (!document.documentMode) return;
  document.documentElement.className += ' legacy-ie';
  if (!window.crypto) window.crypto = window.msCrypto || {};
  if (!window.crypto.randomUUID) window.crypto.randomUUID = function () {
    var bytes = new Uint8Array(16), i;
    if (window.crypto.getRandomValues) window.crypto.getRandomValues(bytes);
    else for (i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
    bytes[6] = (bytes[6] & 15) | 64; bytes[8] = (bytes[8] & 63) | 128;
    var hex = []; for (i = 0; i < 16; i++) hex.push(('0' + bytes[i].toString(16)).slice(-2));
    return hex.slice(0, 4).join('') + '-' + hex.slice(4, 6).join('') + '-' + hex.slice(6, 8).join('') + '-' + hex.slice(8, 10).join('') + '-' + hex.slice(10).join('');
  };
  if (!Element.prototype.matches) Element.prototype.matches = Element.prototype.msMatchesSelector;
  if (!Element.prototype.closest) Element.prototype.closest = function (selector) { var e = this; while (e && e.nodeType === 1) { if (e.matches(selector)) return e; e = e.parentElement; } return null; };
  if (!Element.prototype.remove) Element.prototype.remove = function () { if (this.parentNode) this.parentNode.removeChild(this); };
  if (!Element.prototype.replaceChildren) Element.prototype.replaceChildren = function () { while (this.firstChild) this.removeChild(this.firstChild); for (var i = 0; i < arguments.length; i++) this.appendChild(typeof arguments[i] === 'string' ? document.createTextNode(arguments[i]) : arguments[i]); };
  if (!NodeList.prototype.forEach) NodeList.prototype.forEach = Array.prototype.forEach;
  if (window.DOMTokenList) {
    ['add', 'remove'].forEach(function (name) { var original = DOMTokenList.prototype[name]; DOMTokenList.prototype[name] = function () { for (var i = 0; i < arguments.length; i++) original.call(this, arguments[i]); }; });
    var toggle = DOMTokenList.prototype.toggle;
    DOMTokenList.prototype.toggle = function (name, force) { if (arguments.length > 1) { if (force) this.add(name); else this.remove(name); return !!force; } return toggle.call(this, name); };
  }
  if (!Object.getOwnPropertyDescriptor(Node.prototype, 'isConnected')) Object.defineProperty(Node.prototype, 'isConnected', { get: function () { return document.documentElement.contains(this); } });
  if (!Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'selectedOptions')) Object.defineProperty(HTMLSelectElement.prototype, 'selectedOptions', { get: function () { return this.querySelectorAll('option:checked'); } });
  if (typeof window.Event !== 'function') window.Event = function (name, options) { var event = document.createEvent('Event'); options = options || {}; event.initEvent(name, !!options.bubbles, !!options.cancelable); return event; };
  if (!window.TextDecoder) window.TextDecoder = function () { this.decode = function (bytes) { var encoded = ''; for (var i = 0; i < bytes.length; i++) encoded += '%' + ('0' + bytes[i].toString(16)).slice(-2); return decodeURIComponent(encoded); }; };
  // IE11 does not implement <details>; preserve its open/close user action.
  if (!('open' in document.createElement('details'))) {
    Object.defineProperty(HTMLElement.prototype, 'open', { get: function () { return this.hasAttribute('open'); }, set: function (open) { if (open) this.setAttribute('open', ''); else this.removeAttribute('open'); } });
    document.addEventListener('click', function (event) { var summary = event.target.closest && event.target.closest('summary'); if (summary && summary.parentNode.tagName === 'DETAILS') { summary.parentNode.open = !summary.parentNode.open; event.preventDefault(); } });
  }
  // inert is used only to lock the background while the login modal is open.
  if (!('inert' in HTMLElement.prototype)) Object.defineProperty(HTMLElement.prototype, 'inert', { get: function () { return this.hasAttribute('data-inert'); }, set: function (value) { if (value) this.setAttribute('data-inert', ''); else this.removeAttribute('data-inert'); } });
  document.addEventListener('click', function (event) { if (event.target.closest && event.target.closest('[data-inert]')) { event.preventDefault(); event.stopImmediatePropagation(); } }, true);
  document.addEventListener('focusin', function (event) { if (event.target.closest && event.target.closest('[data-inert]')) event.target.blur(); });
  // IE11 requires type=date to be entered as ISO text; validation stays shared.
  document.addEventListener('DOMContentLoaded', function () {
    var dates = document.querySelectorAll('input[type="date"]');
    for (var i = 0; i < dates.length; i++) { dates[i].setAttribute('placeholder', 'ГГГГ-ММ-ДД'); dates[i].setAttribute('title', 'Дата в формате ГГГГ-ММ-ДД, например 2026-11-06'); }
  });
}());
