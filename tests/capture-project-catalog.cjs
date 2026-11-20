'use strict';
const {launch, stop} = require('./native-packaged-fixture.cjs');
const {login} = require('./statistics-native-helpers.cjs');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
(async () => {
  const out = path.resolve('dist/catalog-screens');
  fs.mkdirSync(out, {recursive:true});
  const run = await launch(process.env.CATALOG_FIXTURE_ROOT);
  try {
    const page = run.page;
    await page.setViewportSize({width:1440,height:1120});
    const {adminId,sourceId} = await page.evaluate(async () => {
      const b = Production.backend, admin = (await b.listAuditUsers()).find(u => u.role === 'admin');
      const cases = [
        ['Капитальный ремонт пресса', 'УЧ-231', 'Ремонт оборудования', 'Механический завод', 8, 4],
        ['Модернизация участка сборки', 'УЧ-232', 'Модернизация', 'Сборочный завод', 6, 2],
        ['Стенд для контроля размеров', 'УЧ-233', 'Оснастка', 'Инструментальный завод', 4, 0],
        ['Приспособление для сборки', 'УЧ-234', 'Оснастка', 'Инструментальный завод', 4, 1]
      ];
      let sourceId;
      for (const [name,orderNo,topic,enterprise,count,done] of cases) {
        const project = {name,orderNo,topic,initiator:'Производственный отдел',executor:'Технический отдел',addressees:enterprise,start:'2026-11-02',deadline:'2026-12-18'};
        const names = ['Подготовить чертёж','Согласовать материал','Изготовить детали','Провести приёмку','Проверить документацию','Согласовать комплект','Провести испытания','Закрыть заказ'];
        const stages = Array.from({length:count}, (_,i) => ({uid:'stage-'+(i+1),parentUid:null,seq:i+1,sort:i+1,title:names[i],executor:'Технический отдел',addressees:enterprise,start:project.start,deadline:project.deadline,status:i<done?'done':'work',comment:''}));
        const evidence = done ? {documentType:'Акт приёмки',documentReference:'УЧ-12 от 18.11.2026',comment:'Этап принят',stageUids:stages.slice(0,done).map(s=>s.uid)} : null;
        const saved = await b.saveSigned({project,stages,nextSeq:count+1},{userId:admin.id,pin:'739201',comment:'Учебный проект',evidence});
        if (orderNo==='УЧ-234') sourceId=saved.projectId;
      }
      return {adminId:admin.id,sourceId};
    });
    await login(page,adminId);
    await page.click('#loadJsonBtn');
    const dialog = page.locator('#projectsModal > .modal');
    await page.waitForSelector('[data-open-project-id="'+sourceId+'"]');
    await page.fill('#catalogQuery','УЧ-');
    assert.equal(await page.locator('.project-list-row').count(),4);
    assert.match(await dialog.innerText(),/Оснастка/);
    await page.evaluate(async()=>{await document.fonts.ready;});
    await dialog.screenshot({path:path.join(out,'01-saved-projects.png')});
    await page.click('[data-copy-project-id="'+sourceId+'"]');
    const field = name => page.locator('#catalogEditor [name="'+name+'"]');
    await field('name').fill('Приспособление для сборки — серия 2');
    await field('order').fill('УЧ-235');
    await field('document').selectOption('Акт приёмки');
    await field('reference').fill('Акт № 12 от 18.11.2026');
    await field('evidenceComment').fill('Чертёж согласован для второй серии');
    assert.equal(await field('topic').inputValue(),'Оснастка');
    assert.equal(await field('pin').inputValue(),'');
    await dialog.screenshot({path:path.join(out,'02-copy-project.png')});
    await page.click('#catalogEditor [data-cancel]');
    await page.click('[data-topic-project-id="'+sourceId+'"]');
    await field('topic').fill('Сборочная оснастка');
    await field('reason').fill('Уточнена тематика проекта');
    assert.equal(await field('pin').inputValue(),'');
    await dialog.screenshot({path:path.join(out,'03-project-topic.png')});
  } catch(e) {
    await run.page.screenshot({path:path.join(out,'diagnostic.png')}).catch(()=>{});
    throw e;
  } finally {await stop(run);}
})().catch(error=>{console.error(error);process.exitCode=1;});