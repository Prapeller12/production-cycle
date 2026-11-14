'use strict';
(function () {
  if (!window.__PRODUCTION_SERVER__ || !window.__PRODUCTION_SERVER__.enabled) return;
  var active = false;
  function invoke(command, args) {
    return new Promise(function (resolve, reject) {
      var request = new XMLHttpRequest();
      request.open('POST', '/api/invoke', true);
      request.timeout = 45000;
      request.setRequestHeader('Content-Type', 'application/json; charset=utf-8');
      request.setRequestHeader('X-Production-Request', '1');
      request.onload = function () {
        var response;
        try { response = JSON.parse(request.responseText); } catch (_) { reject(Error('Сервер вернул некорректный ответ.')); return; }
        if (request.status < 200 || request.status >= 300 || !response.ok) { reject(Error(response.error || 'Не удалось выполнить операцию.')); return; }
        if (command === 'network_login') active = response.result.state === 'active';
        if (command === 'network_status') active = response.result.active;
        if (command === 'network_logout' || command === 'network_finish_eviction') active = false;
        resolve(response.result);
      };
      request.onerror = function () { reject(Error('Нет соединения с сервером. Проверьте сеть и работу серверной программы.')); };
      request.ontimeout = function () { reject(Error('Сервер не ответил вовремя. Проверьте подключение перед повторным сохранением.')); };
      request.send(JSON.stringify({ command: command, args: args || {} }));
    });
  }
  window.ProductionServer = { invoke: invoke };
  window.addEventListener('pagehide', function () {
    if (active && navigator.sendBeacon) navigator.sendBeacon('/api/logout', '');
  });
  window.addEventListener('unload', function () {
    if (active && !navigator.sendBeacon) {
      try { var request = new XMLHttpRequest(); request.open('POST', '/api/logout', false); request.send(''); } catch (_) {}
    }
  });
}());
