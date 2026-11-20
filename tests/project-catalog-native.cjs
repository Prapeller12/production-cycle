'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const {launch, stop, pristineRoot} = require('./native-packaged-fixture.cjs');
const {login} = require('./statistics-native-helpers.cjs');

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
        addressees:'Предприятие', start:'2026-08-01', deadline:'2026-12-01'};
      const stage = {uid:'parent',seq:1,sort:1,parentUid:null,title:'Первая операция',executor:'Исполнитель',
        addressees:'Цех',start:project.start,deadline:project.deadline,status:'done',comment:'Проверен материал'};
      const source = {project,stages:[stage,{...stage,uid:'child',seq:2,parentUid:'parent',title:'Вторая операция',status:'work'}],nextSeq:3};
      const result = await b.saveSigned(source,{userId:admin.id,pin:'739201',comment:'Исходный проект',
        evidence:{documentType:'Акт приёмки',documentReference:'К-1',comment:'Испытание исходника',stageUids:['parent']}});
      return {admin:admin.id, id:result.projectId};
    });
    await login(page, fixture.admin);
    await page.fill('#projectName','Несохранённый черновик QA');
    await page.click('#loadJsonBtn');
    const field = name => page.locator('#catalogEditor [name="'+name+'"]');
    const submit = () => page.click('#catalogEditor [type=submit]');
    const row = id => page.locator('.project-list-row').filter({has:page.locator('[data-open-project-id="'+id+'"]')});
    await page.click('[data-topic-project-id="'+fixture.id+'"]');
    await field('topic').fill('Оснастка');
    await field('reason').fill('Объединение по тематике');
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
    await field('topic').fill('Ремонт'); await field('document').selectOption('Акт приёмки');
    await field('reference').fill('К-2'); await field('evidenceComment').fill('Подтверждено для копии');
    await field('pin').fill('неверный'); await submit();
    await page.waitForSelector('#catalogNotice.err'); await field('pin').fill('739201'); await submit();
    await page.waitForSelector('#catalogEditor',{state:'hidden'});
    await page.waitForSelector('[data-copy-project-id="'+fixture.id+'"]');
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
    for(const [i, original] of check.a.stages.entries()){
      const duplicate = check.b.stages[i];assert.notEqual(duplicate.uid, original.uid);
      for(const key of ['seq','sort','title','start','deadline','executor','addressees','comment','status'])assert.equal(duplicate[key],original[key],key);
    }
    assert.equal(check.verification.invalidEvents,0);
    assert.deepEqual(check.events.map(e=>e.eventType).sort(),['project_created','stage_completed']);
    assert.ok(check.sourceEvents.some(e=>e.changes.some(c=>c.field==='Тематика')));
    await page.click('[data-topic-project-id="'+copy+'"]');await field('topic').fill('Отмена');
    await page.click('#catalogEditor [data-cancel]');assert.equal(await page.locator('#catalogEditor input[type=password]').count(),0);
    await page.click('#projectsModal .modal-foot [data-close]');assert.equal(await page.locator('#projectName').inputValue(),'Несохранённый черновик QA');
    await stop(run);run=await launch(work);await login(run.page,fixture.admin);await run.page.click('#loadJsonBtn');
    await run.page.waitForSelector('[data-open-project-id="'+copy+'"]');
    await run.page.selectOption('#catalogTopic','topic:ремонт');assert.equal(await run.page.locator('[data-open-project-id]').count(),1);
    assert.equal(await run.page.locator('[data-open-project-id]').getAttribute('data-open-project-id'),String(copy));
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(evidenceDir,'project-catalog-native.json'),JSON.stringify({ok:true,copyIndependent:true,treeAndStatuses:true,topicAudit:true,filterPersistsDataAfterRestart:true,openDraftPreserved:true},null,2));
    console.log('PASS saved project copy, signed topics, independent history, search and restart');
  } catch (error) {
    await run?.page?.screenshot({path:path.join(evidenceDir,'project-catalog-failure.png')}).catch(()=>{});
    throw error;
  } finally {await stop(run);}
})().catch(error=>{console.error(error);process.exitCode=1;});
