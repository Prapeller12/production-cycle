'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const {launch, stop, pristineRoot} = require('./native-packaged-fixture.cjs');
const {login} = require('./statistics-native-helpers.cjs');

async function checkCatalogLayout(page, evidenceDir) {
  const dialog = page.locator('#projectsModal > .modal');
  await page.evaluate(() => document.fonts.ready);
  for (const width of [1440, 1101, 1024, 900, 850, 768, 520, 375, 320]) {
    await page.setViewportSize({width, height:960});
    const measurements = await dialog.evaluate(modal => {
      const list = modal.querySelector('#projectsList');
      const rows = [...list.querySelectorAll('.project-list-row')];
      const rightEdge = list.getBoundingClientRect().left + list.clientLeft + list.clientWidth;
      const head = list.querySelector('.project-list-head');
      const headers = head.getClientRects().length ? [...head.children] : [];
      const clipped = rows.flatMap(row => [...row.querySelectorAll('button')].filter(button => {
        const box = button.getBoundingClientRect();
        return box.right > Math.min(rightEdge, row.getBoundingClientRect().right) + 1 ||
          box.left < row.getBoundingClientRect().left - 1 || button.scrollWidth > button.clientWidth + 1;
      }).map(button => button.textContent));
      const offsets = rows.flatMap(row => headers.map((header, i) =>
        Math.abs(header.getBoundingClientRect().left - row.children[i].getBoundingClientRect().left)));
      return {
        rows:rows.length, buttons:rows.map(row => row.querySelectorAll('button').length),
        overflow:[modal,list,...rows].map(el => el.scrollWidth - el.clientWidth),
        clipped, alignment:Math.max(0, ...offsets)
      };
    });
    assert.ok(measurements.rows > 1, 'Expected projects at width ' + width);
    assert.ok(measurements.buttons.every(count => count === 3), 'Missing catalogue actions at ' + width);
    assert.ok(measurements.overflow.every(px => px <= 1), 'Horizontal overflow at ' + width + ': ' + measurements.overflow);
    assert.deepEqual(measurements.clipped, [], 'Clipped controls at ' + width);
    assert.ok(measurements.alignment <= 1, 'Misaligned header at ' + width);
  }
  await page.setViewportSize({width:1440,height:960});
  await page.fill('#catalogQuery','УЧ-');
  assert.equal(await page.locator('.project-list-row').count(), 4);
  await dialog.screenshot({path:path.join(evidenceDir,'saved-projects-columns.png')});
  await page.fill('#catalogQuery','');
}

(async () => {
  const repo = path.resolve(__dirname, '..');
  const work = pristineRoot(repo);
  const evidenceDir = path.join(repo, 'dist/local-window-qa');
  fs.mkdirSync(evidenceDir, {recursive: true});
  let run = await launch(work);
  try {
    const page = run.page, errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const fixture = await page.evaluate(async () => {
      const b = Production.backend, admin = (await b.listAuditUsers()).find(u => u.role === 'admin');
      const project = {name:'Каталог исходный', orderNo:'CAT-001', initiator:'Инициатор', executor:'Координатор',
        addressees:'ПроизводственноеПредприятие'.repeat(3), start:'2026-08-01', deadline:'2026-12-01'};
      const stage = {uid:'parent',seq:1,sort:1,parentUid:null,title:'Первая операция',executor:'Исполнитель',
        addressees:'Цех',start:project.start,deadline:project.deadline,status:'done',comment:'Проверен материал'};
      const source = {project,stages:[stage,{...stage,uid:'child',seq:2,parentUid:'parent',title:'Вторая операция',status:'work'}],nextSeq:3};
      const result = await b.saveSigned(source,{userId:admin.id,pin:'739201',comment:'Исходный проект',
        evidence:{documentType:'Акт приёмки',documentReference:'К-1',comment:'Испытание исходника',stageUids:['parent']}});
      const examples = [
        ['Капитальный ремонт пресса','Ремонт оборудования','Механический завод',4,8],
        ['Модернизация участка сборки','Модернизация','Сборочный завод',2,6],
        ['Стенд для контроля размеров','Оснастка','Инструментальный завод',0,4],
        ['Приспособление для сборки','Оснастка','Инструментальный завод',1,4]
      ];
      for (const [i, [name,topic,enterprise,done,total]] of examples.entries()) {
        const stages = Array.from({length:total}, (_,j) => ({
          ...stage, uid:'s'+j, seq:j+1, sort:j+1, title:'Операция '+(j+1), status:j<done?'done':'work'
        }));
        await b.saveSigned({
          project:{...project,name,topic,orderNo:'УЧ-'+(231+i),addressees:enterprise,deadline:'2026-12-18'},
          stages,nextSeq:total+1
        }, {userId:admin.id,pin:'739201',comment:'Учебный проект',evidence:done ? {
          documentType:'Акт приёмки',documentReference:'УЧ-12',comment:'Этапы приняты',stageUids:stages.slice(0,done).map(s=>s.uid)
        } : null});
      }
      return {admin:admin.id, id:result.projectId};
    });
    await login(page, fixture.admin);
    await page.fill('#projectName','Несохранённый черновик QA');
    await page.click('#loadJsonBtn');
    await page.waitForSelector('[data-open-project-id="'+fixture.id+'"]');
    await checkCatalogLayout(page, evidenceDir);
    const field = name => page.locator('#catalogEditor [name="'+name+'"]');
    const submit = () => page.click('#catalogEditor [type=submit]');
    const row = id => page.locator('.project-list-row').filter({has:page.locator('[data-open-project-id="'+id+'"]')});
    await page.click('[data-topic-project-id="'+fixture.id+'"]');
    await field('topic').fill('Оснастка');
    assert.equal(await field('reason').inputValue(), '');
    assert.equal(await field('reason').evaluate(el => el.required), false);
    await field('pin').fill('739201');
    await submit();
    await page.waitForSelector('#catalogEditor', {state:'hidden'});
    assert.match(await row(fixture.id).innerText(), /Оснастка/);
    await page.selectOption('#catalogTopic','none');
    assert.equal(await row(fixture.id).count(),0);
    await page.selectOption('#catalogTopic','topic:оснастка');
    await page.fill('#catalogQuery','КАТАЛОГ');
    assert.equal(await row(fixture.id).count(),1);
    await page.click('[data-copy-project-id="'+fixture.id+'"]');
    await field('name').fill('Каталог копия'); await field('order').fill('CAT-002');
    assert.equal(await field('topic').inputValue(),'Оснастка');
    await field('topic').fill('Ремонт');
    assert.equal(await page.locator('#catalogEditor fieldset, #catalogEditor [name="document"], #catalogEditor [name="reference"], #catalogEditor [name="evidenceComment"]').count(), 0);
    assert.equal(await field('reason').inputValue(), '');
    assert.equal(await field('reason').evaluate(el => el.required), false);
    await field('reason').fill('   ');
    await field('pin').fill('неверный'); await submit();
    await page.waitForSelector('#catalogNotice.err'); await field('pin').fill('739201');
    const beforeCopy = await page.evaluate(() => Production.domain.today());
    await submit();
    await page.waitForSelector('#catalogEditor',{state:'hidden'});
    await page.waitForSelector('[data-copy-project-id="'+fixture.id+'"]');
    await page.selectOption('#catalogTopic','topic:ремонт');
    await page.fill('#catalogQuery','ремОнТ');
    assert.equal(await page.locator('.project-list-row').count(),1);
    const copy = Number(await page.locator('[data-open-project-id]').getAttribute('data-open-project-id'));
    assert.notEqual(copy,fixture.id);
    const check = await page.evaluate(async ({fixture,copy}) => {
      const b = Production.backend;
      return {a:await b.loadById(fixture.id),b:await b.loadById(copy),events:await b.listAuditEvents(copy),
        sourceEvents:await b.listAuditEvents(fixture.id),verification:await b.verifyAuditLog(fixture.admin,'739201')};
    },{fixture,copy});
    assert.equal(check.a.project.topic,'Оснастка');assert.equal(check.b.project.topic,'Ремонт');
    assert.equal(check.b.project.orderNo,'CAT-002');assert.equal(check.b.project.name,'Каталог копия');
    assert.equal(check.b.stages[1].parentUid, check.b.stages[0].uid);
    const afterCopy = await page.evaluate(() => Production.domain.today());
    const closed = check.a.stages[0], reopened = check.b.stages[0];
    assert.equal(closed.status, 'done'); assert.equal(closed.deadline,'2026-12-01');
    assert.equal(reopened.status,'new');
    assert.ok([beforeCopy,afterCopy].some(day => Date.parse(reopened.deadline) - Date.parse(day) === 30 * 86400000));
    assert.equal(check.b.stages[1].status, 'work'); assert.equal(check.b.stages[1].deadline, check.a.stages[1].deadline);
    for(const [i, original] of check.a.stages.entries()){
      const duplicate = check.b.stages[i];assert.notEqual(duplicate.uid, original.uid);
      for(const key of ['seq','sort','title','start','executor','addressees','comment'])assert.equal(duplicate[key],original[key],key);
    }
    assert.equal(check.verification.invalidEvents,0);
    assert.deepEqual(check.events.map(e=>e.eventType), ['project_created']);
    assert.equal(check.events[0].comment, 'Автоматическая запись. Копирование проекта № ' + fixture.id);
    assert.ok(check.sourceEvents.some(e => e.eventType === 'stage_completed'));
    assert.ok(check.sourceEvents.some(e => e.comment === 'Автоматическая запись. Изменение тематики проекта № ' + fixture.id));
    assert.ok(check.sourceEvents.some(e=>e.changes.some(c=>c.field==='Тематика')));
    await page.click('[data-topic-project-id="'+copy+'"]');await field('topic').fill('Отмена');
    await page.click('#catalogEditor [data-cancel]');assert.equal(await page.locator('#catalogEditor input[type=password]').count(),0);
    await page.click('#projectsModal .modal-foot [data-close]');assert.equal(await page.locator('#projectName').inputValue(),'Несохранённый черновик QA');
    await stop(run);run=await launch(work);await login(run.page,fixture.admin);await run.page.click('#loadJsonBtn');
    await run.page.waitForSelector('[data-open-project-id="'+copy+'"]');
    await run.page.selectOption('#catalogTopic','topic:ремонт');assert.equal(await run.page.locator('[data-open-project-id]').count(),1);
    assert.equal(await run.page.locator('[data-open-project-id]').getAttribute('data-open-project-id'),String(copy));
    await run.page.click('[data-topic-project-id="'+copy+'"]');
    await run.page.locator('#catalogEditor [name="topic"]').fill('Оснастка');
    await run.page.locator('#catalogEditor [name="reason"]').fill('Перенос в другую тематику');
    await run.page.locator('#catalogEditor [name="pin"]').fill('739201');
    await run.page.click('#catalogEditor [type=submit]');
    await run.page.waitForSelector('#catalogEditor', {state:'hidden'});
    const events = await run.page.evaluate(id => Production.backend.listAuditEvents(id), copy);
    assert.ok(events.some(e => e.comment === 'Перенос в другую тематику'));
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(evidenceDir,'project-catalog-native.json'),JSON.stringify({ok:true,copyIndependent:true,treeAndOpenStageStatuses:true,closedStagesReset:true,deadlinePlus30CalendarDays:true,optionalOperationComments:true,topicAudit:true,filterPersistsDataAfterRestart:true,openDraftPreserved:true},null,2));
    console.log('PASS saved project copy, signed topics, independent history, search and restart');
  } catch (error) {
    await run?.page?.screenshot({path:path.join(evidenceDir,'project-catalog-failure.png')}).catch(()=>{});
    throw error;
  } finally {await stop(run);}
})().catch(error=>{console.error(error);process.exitCode=1;});
