'use strict';
(() => {
  const key = text => String(text || '').trim().toLocaleLowerCase('ru');
  const {copiedClosedStageDays} = Production.config;
  function copyProject(source, name, orderNo, topic, newUid, copyDate = Production.domain.today()) {
    const ids = new Map(source.stages.map(stage => [stage.uid, newUid()]));
    // Start with the local calendar date of submission. UTC arithmetic avoids DST offsets.
    const due = new Date(copyDate + 'T00:00:00Z');
    due.setUTCDate(due.getUTCDate() + copiedClosedStageDays);
    const deadline = due.toISOString().slice(0, 10);
    return {
      databaseId: null, baseRevision: null, nextSeq: source.nextSeq, collapsed: {},
      project: { ...source.project, name, orderNo, topic },
      stages: source.stages.map(stage => {
        const duplicate = {...stage, uid: ids.get(stage.uid), parentUid: stage.parentUid == null ? null : ids.get(stage.parentUid)};
        if (stage.status === 'done') {
          duplicate.status = 'new';
          duplicate.deadline = deadline;
          if (duplicate.start > deadline) duplicate.start = copyDate;
        }
        return duplicate;
      })
    };
  }

  function create({ $, backend, renderRows, currentUser, epoch, newUid, topicSaved }) {
    const editor = $('catalogEditor'), list = $('projectsList'), filters = $('catalogFilters');
    const query = $('catalogQuery'), filter = $('catalogTopic'), notice = $('catalogNotice');
    let projects = [], topics = [], generation = 0, saving = false;
    const isCurrent = (ticket, session) => ticket === generation && session === epoch() && $('projectsModal').classList.contains('open');
    function notify(text, kind = 'info') {
      notice.textContent = text;
      notice.className = text ? 'notice ' + kind : '';
    }
    function clear() {
      generation++;
      saving = false;
      editor.replaceChildren();
      editor.hidden = true;
      filters.hidden = list.hidden = false;
      notify('');
    }
    function applyFilters() {
      const term = key(query.value);
      const matches = projects.filter(project => {
        const topic = key(project.topic);
        if (filter.value === 'none' && topic) return false;
        if (filter.value.startsWith('topic:') && filter.value.slice(6) !== topic) return false;
        return [project.name, project.orderNo, project.enterprise, project.topic].some(value => key(value).includes(term));
      });
      renderRows(matches, projects.length);
    }
    function updateTopics() {
      const previous = filter.value;
      const unique = new Map();
      projects.forEach(p => { if (key(p.topic)) unique.set(key(p.topic), p.topic.trim()); });
      topics = [...unique.values()].sort((a, b) => a.localeCompare(b, 'ru'));
      filter.replaceChildren(new Option('Все тематики', 'all'), new Option('Без тематики', 'none'));
      topics.forEach(topic => filter.add(new Option(topic, 'topic:' + key(topic))));
      if ([...filter.options].some(option => option.value === previous)) filter.value = previous;
    }
    async function refresh() {
      const ticket = ++generation, session = epoch();
      list.textContent = 'Загрузка…';
      try {
        const items = await backend.listProjects();
        if (!isCurrent(ticket, session)) return;
        projects = items;
        updateTopics();
        applyFilters();
      } catch (error) {
        if (isCurrent(ticket, session)) {
          projects = [];
          list.replaceChildren();
          notify('Не удалось обновить список проектов. ' + error, 'err');
        }
      }
    }
    async function show(initialQuery) {
      clear();
      query.value = initialQuery;
      filter.replaceChildren();
      await refresh();
    }
    function returnToList(id, action) {
      clear();
      applyFilters();
      list.querySelector('[data-' + action + '-project-id="' + id + '"]')?.focus();
    }
    async function edit(id, copying) {
      if (saving) return;
      const ticket = ++generation, session = epoch();
      notify('Загрузка проекта…');
      try {
        const [source, users] = await Promise.all([backend.loadById(id), backend.listAuditUsers()]);
        if (!isCurrent(ticket, session)) return;
        if (!source) throw Error('Проект больше не найден.');
        const resetsStages = copying && source.stages.some(stage => stage.status === 'done');
        const title = copying ? 'Копировать проект' : 'Изменить тематику';
        const field = (label, input) => '<label>' + label + input + '</label>';
        editor.innerHTML = '<form class="catalog-form"><h3 tabindex="-1">' + title + '</h3><p data-source></p><div class="catalog-grid">' +
          (copying ? field('Название копии', '<input name="name" required>') + field('Номер заказа', '<input name="order" required>') : '') +
          field('Тематика', '<input name="topic" list="catalogKnownTopics" maxlength="120" placeholder="Выберите или введите новую">') +
          '<datalist id="catalogKnownTopics"></datalist></div>' +
          (resetsStages ? '<p>В копии выполненные этапы получат статус «Не начато». Срок исполнения — через ' + copiedClosedStageDays + ' календарных дней со дня копирования.</p>' : '') +
          '<div class="catalog-grid">' + field('Подписывает', '<select name="signer" required></select>') +
          field('ПИН подписывающего', '<input name="pin" type="password" autocomplete="off" required>') +
          '</div>' + field('Комментарий к записи в журнале (необязательно)', '<input name="reason" maxlength="2000">') +
          '<div class="catalog-actions"><button type="submit" class="btn primary">' +
          (copying ? 'Подписать и создать копию' : 'Подписать и сохранить тематику') +
          '</button><button type="button" class="btn" data-cancel>Отмена</button></div></form>';
        const form = editor.querySelector('form'), fields = form.elements;
        editor.querySelector('[data-source]').textContent = 'Проект «' + source.project.name + '», заказ № ' + source.project.orderNo + ' · № ' + id;
        if (copying) {
          fields.namedItem('name').value = source.project.name + ' — копия';
          fields.order.value = source.project.orderNo;
        }
        fields.topic.value = source.project.topic || '';
        for (const topic of topics) $('catalogKnownTopics').append(new Option(topic, topic));
        for (const u of users.filter(u => u.active)) fields.signer.add(new Option(u.displayName, String(u.id)));
        if ([...fields.signer.options].some(o => o.value === String(currentUser()))) fields.signer.value = String(currentUser());
        filters.hidden = list.hidden = true;
        editor.hidden = false;
        notify('');
        editor.querySelector('h3').focus();
        editor.querySelector('[data-cancel]').onclick = () => returnToList(id, copying ? 'copy' : 'topic');
        form.onsubmit = async event => {
          event.preventDefault();
          if (saving || !isCurrent(ticket, session)) return;
          const raw = fields.topic.value.trim(), topic = topics.find(t => key(t) === key(raw)) || raw;
          if (!copying && topic === (source.project.topic || '')) return notify('Тематика не изменилась.', 'info');
          const model = copying
            ? copyProject(source, fields.namedItem('name').value.trim(), fields.order.value.trim(), topic, newUid)
            : { ...source, project: { ...source.project, topic } };
          const action = copying ? 'Копирование проекта № ' : 'Изменение тематики проекта № ';
          const comment = fields.reason.value.trim() || 'Автоматическая запись. ' + action + id;
          const confirmation = {userId: Number(fields.signer.value), pin: fields.pin.value, comment, evidence: null};
          saving = true;
          for (const el of fields) el.disabled = true;
          let saved;
          try {
            saved = await backend.saveSigned(model, confirmation);
          } catch (error) {
            if (isCurrent(ticket, session)) notify(String(error), 'err');
            return;
          } finally {
            if (isCurrent(ticket, session)) {
              fields.pin.value = '';
              for (const el of fields) el.disabled = false;
              saving = false;
            }
          }
          if (!isCurrent(ticket, session)) return;
          if (!copying) topicSaved(source, topic, saved.baseRevision);
          clear();
          // Reveal the saved row even when it has just left the old filter.
          query.value = '';
          filter.value = 'all';
          notify((copying ? 'Создана копия' : 'Тематика сохранена') + ' · проект № ' + saved.projectId +
            (saved.backupWarning ? '. ' + saved.backupWarning : '.'), saved.backupWarning ? 'warn' : 'ok');
          await refresh();
          const row = list.querySelector('[data-open-project-id="' + saved.projectId + '"]');
          if (session === epoch() && editor.hidden && $('projectsModal').classList.contains('open')) row?.focus();
        };
      } catch (error) {
        if (isCurrent(ticket, session)) notify('Не удалось открыть проект. ' + error, 'err');
      }
    }
    list.addEventListener('click', event => {
      const button = event.target.closest('[data-copy-project-id],[data-topic-project-id]');
      if (button) void edit(Number(button.dataset.copyProjectId || button.dataset.topicProjectId), button.hasAttribute('data-copy-project-id'));
    });
    query.oninput = applyFilters;
    filter.onchange = applyFilters;
    return {show, clear, isSaving: () => saving};
  }
  window.ProductionProjectCatalog = {create, copyProject};
})();
