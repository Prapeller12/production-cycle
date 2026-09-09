const assert=require('node:assert/strict');
const path=require('node:path');
const fs=require('node:fs');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[],remote=[];
  page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(/^https?:/.test(r.url()))remote.push(r.url())});
  page.on('dialog',d=>d.accept());
  await page.goto(pathToFileURL(path.resolve(__dirname,'../frontend/index.html')).href);
  await page.click('#demoBtn');
  assert.equal(await page.locator('#treeBody tr').count(),8); // root + 7 active; completed is hidden by default
  await page.selectOption('#completionFilter','all');assert.equal(await page.locator('#treeBody tr').count(),9);
  await page.selectOption('#completionFilter','done');assert.equal(await page.locator('#treeBody tr').count(),2);
  await page.selectOption('#completionFilter','all');
  await page.click('[data-view="gantt"]');assert.equal(await page.locator('#ganttChart .gantt-bar').count(),8);await page.click('[data-view="registry"]');
  await page.click('#validateBtn');assert.match(await page.locator('#messageArea').innerText(),/Проверка пройдена/);
  await page.click('#exportBtn');assert.ok((await page.locator('#txtPreview').innerText()).includes('924'));
  await page.click('[data-close="exportModal"]');
  const download=page.waitForEvent('download');await page.click('#saveJsonBtn');const d=await download;
  const saved=JSON.parse(fs.readFileSync(await d.path(),'utf8').replace(/^\ufeff/,''));assert.equal(saved.stages.length,8);
  await page.locator('#jsonFile').setInputFiles({name:'broken.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify({...saved,stages:[{...saved.stages[0],parentUid:saved.stages[0].uid}]}))});
  await page.waitForFunction(()=>document.querySelector('#messageArea').innerText.includes('Не удалось открыть'));
  assert.match(await page.locator('#messageArea').innerText(),/Не удалось открыть/);assert.equal(await page.locator('#treeBody tr').count(),9);
  await page.locator('#treeBody [data-act="edit"]').first().click();await page.fill('#stageTitle','Этап после редактирования');await page.click('#saveStageBtn');
  assert.match(await page.locator('#treeBody').innerText(),/Этап после редактирования/);
  await page.evaluate(()=>{window.print=()=>{window.__printed=true}});await page.click('#pdfReportBtn');await page.waitForFunction(()=>window.__printed);
  assert.equal(await page.locator('#pdfReport .report-stage-table tbody tr').count(),8);
  const pair=await page.evaluate(()=>Production.exchange.exportPair({project:{name:'Импорт',orderNo:'777',initiator:'Инициатор',executor:'Исполнитель',addressees:'Предприятие',start:'2026-09-01',deadline:'2026-09-30'},stages:[{uid:'i1',seq:1,sort:1,parentUid:null,title:'Входящий этап',executor:'Исполнитель',addressees:'Предприятие',start:'2026-09-01',deadline:'2026-09-20',status:'work'}],nextSeq:2,collapsed:{}}));
  await page.click('#importBtn');await page.locator('#importOutFile').setInputFiles({name:'Список исх.txt',mimeType:'text/plain',buffer:Buffer.from('\ufeff'+pair.out)});await page.locator('#importInFile').setInputFiles({name:'Список вхд.txt',mimeType:'text/plain',buffer:Buffer.from('\ufeff'+pair.in)});await page.waitForSelector('#confirmImportBtn:not([disabled])');await page.click('#confirmImportBtn');assert.equal(await page.inputValue('#orderNo'),'777');assert.match(await page.locator('#treeBody').innerText(),/Входящий этап/);
  await page.screenshot({path:path.join(process.env.PROTOTYPE_QA_DIR||'/tmp','production-cycle-desktop.png'),fullPage:true});
  await page.emulateMedia({media:'print'});await page.pdf({path:path.join(process.env.PROTOTYPE_QA_DIR||'/tmp','production-cycle-report.pdf'),preferCSSPageSize:true,printBackground:true});await page.emulateMedia({media:'screen'});
  await page.setViewportSize({width:760,height:900});await page.screenshot({path:path.join(process.env.PROTOTYPE_QA_DIR||'/tmp','production-cycle-narrow.png'),fullPage:true});
  assert.deepEqual(errors,[]);assert.deepEqual(remote,[]);
  console.log('Browser: demo, validation, JSON/TXT, failed import preservation, edit, PDF and zero network requests passed.');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exit(1)});
