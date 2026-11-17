'use strict';
// Called only by local-window.cjs against the actual packaged EXE, SQLite and native IPC.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {S,action,widget,openStatistics,edit,cancel,save,preferences,showAllWidgets,seed,setFilter,width,expectSaved}=require('./statistics-native-helpers.cjs');
module.exports=async(page,out,root)=>{
 assert.equal(await page.locator('#ganttTab').evaluate(e=>e.nextElementSibling.id),'statisticsTab');
 const fixture=await seed(page);await openStatistics(page);
 assert.equal(await page.locator(S+'[data-preset],'+S+'[data-action="save-view"],'+S+'[data-control="participationOrder"]').count(),0,'One editor and one global order filter');
 await setFilter(page,'orderNo','');
 assert.equal(await page.locator(widget('participation')+' tbody tr').count(),0,'All orders does not silently select the first order');
 assert.match(await page.locator(widget('participation')).innerText(),/выберите заказ/i);
 const before=await preferences(page),originalOrder=await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget));
 await edit(page);await page.locator(S+'[data-widget-toggle="enterprises"]').uncheck();
 await page.locator(S+'[data-widget-width="participation"]').selectOption('full');
 await page.click(action('move-down','participation'));
 assert.equal(await page.locator(action('move-down','participation')).evaluate(e=>e===document.activeElement),true,'Move preserves keyboard focus on the same block');
 await page.click('#registryTab');await page.waitForSelector(action('leave-continue'));await page.click(action('leave-continue'));assert.equal(await page.locator('#statisticsTab').getAttribute('aria-selected'),'true','Continue editing keeps Statistics active');
 await cancel(page);
 assert.deepEqual(await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget)),originalOrder,'Cancel restores confirmed layout');
 assert.deepEqual((await preferences(page)).settings,before.settings,'Cancel performs no confirmed native settings write');
 await edit(page);await page.click(action('move-down','participation'));await page.click(action('move-up','participation'));
 assert.equal(await page.evaluate(()=>document.activeElement?.closest('[data-drop-widget]')?.dataset.dropWidget),'participation','Moving to the first position preserves focus within the same block');assert.equal(await page.evaluate(()=>!!document.activeElement?.disabled),false,'Boundary move never leaves focus on a disabled control');
 await page.locator(S+'[data-widget-toggle="enterprises"]').uncheck();
 const draftBeforeDrag=await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget)),transfer=await page.evaluateHandle(()=>new DataTransfer());
 await page.locator(widget('participation')+' [data-drag="participation"]').dispatchEvent('dragstart',{dataTransfer:transfer});await page.keyboard.press('Escape');
 assert.equal(await page.locator(action('save-settings')).count(),1,'Escape during a drag keeps the layout editor open');assert.deepEqual(await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget)),draftBeforeDrag,'Cancelling a drag preserves the existing draft');
 await page.locator(widget('participation')+' [data-drag="participation"]').dispatchEvent('dragend',{dataTransfer:transfer});await transfer.dispose();
 await page.keyboard.press('Escape');await page.waitForSelector(action('save-settings'),{state:'detached'});assert.equal(await page.locator(action('save-settings')).count(),0,'Escape outside a drag cancels the layout draft');assert.deepEqual((await preferences(page)).settings,before.settings);
 await showAllWidgets(page);assert.equal(await page.locator(S+'.pc-widget').count(),9,'Every existing widget remains available');
 await edit(page);await page.dragAndDrop(widget('participation')+' [data-drag="participation"]',widget('enterprises'));assert.notEqual((await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget)))[0],'participation','Dragging a visible block header reorders its preview');await cancel(page);
 await edit(page);await page.click(action('move-down','participation'));
 await page.evaluate(()=>window.__buttonDownloads=[]);await page.click(action('export-settings'));await page.waitForFunction(()=>window.__buttonDownloads.some(x=>x.name==='statistics-settings.json'));
 const exportedSettings=await page.evaluate(()=>JSON.parse(window.__buttonDownloads.find(x=>x.name==='statistics-settings.json').text.replace(/^\uFEFF/,'')));assert.equal(exportedSettings.version,2);assert.notEqual(exportedSettings.widgets[0].id,'participation');await save(page);
 const saved=await expectSaved(page);assert.notEqual(saved.settings.widgets[0].id,'participation','Explicit save commits reordered layout');
 await page.click('#registryTab');await openStatistics(page);
 assert.equal((await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>e.dataset.widget)))[0],saved.settings.widgets.find(w=>w.visible).id);
 await setFilter(page,'orderNo',fixture.order);
 // Half-width previews have equal geometry and at most three records, while full width preserves the old complete bar table.
 const previewRows=page.locator(widget('participation')+' tbody tr, '+widget('participation')+' .pc-overview-list > article');assert.equal(await previewRows.count(),3);
 await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 const sizes=await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.filter(e=>!e.classList.contains('full')).map(e=>{const r=e.getBoundingClientRect();return {width:r.width,height:r.height}}));
 assert.ok(sizes.length>=2);assert.ok(Math.max(...sizes.map(s=>s.width))-Math.min(...sizes.map(s=>s.width))<2,'Half cards have equal widths');
 assert.ok(Math.max(...sizes.map(s=>s.height))-Math.min(...sizes.map(s=>s.height))<2,'Half cards have equal heights');
 await page.click(action('show-all','participation'));assert.equal(await page.locator(S+'.pc-participation-table tbody tr').count(),5);
 await page.click(action('back-overview'));await page.waitForFunction(()=>document.activeElement?.dataset.focus==='show-all-participation');assert.equal(await previewRows.count(),3);assert.equal(await page.locator(action('show-all','participation')).evaluate(e=>e===document.activeElement),true,'Full table returns focus to its trigger');
 await width(page,'participation','full');assert.equal(await previewRows.count(),5,'Full-width participation displays all five performers');
 assert.equal(await page.locator(widget('participation')+' .pc-share-track').count(),5,'Full-width table retains assignment bars');
 const rows=await previewRows.evaluateAll(nodes=>nodes.map(r=>r.textContent));
 const completed=rows.find(text=>text.includes('Исполнитель QA 1'));assert.match(completed,/20\s*%/);assert.match(completed,/100\s*%/);assert.match(completed,/1\s*(?:из|\/)\s*5/);assert.match(completed,/1\s*(?:из|\/)\s*1/);
 await setFilter(page,'executor','Исполнитель QA 1');assert.equal(await previewRows.count(),5,'Performer filter highlights without changing the denominator');
 const highlight=await page.locator(widget('participation')+' tbody tr').filter({hasText:'Исполнитель QA 1'}).getAttribute('class');assert.match(highlight||'',/highlight|selected/);
 const asOf='2026-11-14';await page.locator(S+'[data-filter="asOf"]').fill(asOf);await page.locator(S+'[data-filter="asOf"]').dispatchEvent('change');
 await page.click(action('refresh'));await page.waitForFunction(()=>!document.querySelector('#statisticsMount [data-action="refresh"]').disabled);
 assert.equal(await page.locator(S+'[data-filter="orderNo"]').inputValue(),fixture.order);assert.equal(await page.locator(S+'[data-filter="executor"]').inputValue(),'Исполнитель QA 1');assert.equal(await page.locator(S+'[data-filter="asOf"]').inputValue(),asOf);
 // Input rerenders must not lose the caret or focus after the first character.
 await setFilter(page,'executor','');const search=page.locator(S+'[data-filter="search"]');await search.fill('');await search.pressSequentially('STAT-QA',{delay:15});assert.equal(await search.inputValue(),'STAT-QA');assert.equal(await search.evaluate(e=>e===document.activeElement),true);await search.fill('');
 await edit(page);await page.locator(S+'[data-setting="font"]').focus();await page.locator(S+'[data-setting="font"]').selectOption('large');assert.equal(await page.locator(S+'[data-setting="font"]').evaluate(e=>e===document.activeElement),true);await save(page);
 await width(page,'participation','half');await setFilter(page,'orderNo',fixture.largeOrder);
 assert.equal(await previewRows.count(),3);await page.click(action('show-all','participation'));assert.equal(await page.locator(S+'.pc-participation-table tbody tr').count(),4,'All four performers are accessible beyond the three-card preview');await page.click(action('back-overview'));
 await page.click(action('show-all','risks'));assert.equal(await page.locator(widget('risks')+' tbody tr').count(),100,'All one hundred stage records are accessible in the full table');
 // CSV uses all matching rows, irrespective of three-record preview or table pagination.
 await page.evaluate(()=>window.__buttonDownloads=[]);await page.click(action('export-csv'));
 if(await page.locator(action('confirm-csv')).count())await page.click(action('confirm-csv'));
 await page.waitForFunction(()=>window.__buttonDownloads.some(x=>x.name.endsWith('.csv')));
 const csv=await page.evaluate(()=>window.__buttonDownloads.find(x=>x.name.endsWith('.csv')));assert.ok(csv.text.startsWith('\uFEFF'));assert.equal(csv.text.split('\uFEFF').length-1,1);assert.equal(csv.text.split('\r\n').length,101);assert.ok(csv.text.includes('statistics-stage-99'));await page.click(action('back-overview'));
 // Native export policy validation still applies after the redesign.
 const rejected=await page.evaluate(async()=>{const result=[];for(const [name,text]of [['../escape.csv','x'],['bad.exe','x'],['bad.json','{}']]){try{await window.__nativeInvoke('save_statistics_file',{name,text});result.push(false)}catch{result.push(true)}}return result});assert.deepEqual(rejected,[true,true,true]);
 await setFilter(page,'orderNo',fixture.order);await page.evaluate(()=>{window.__printSignals=0;window.print=()=>window.__printSignals++});
 await page.click(action('print'));await page.waitForFunction(()=>window.__printSignals===1);
 await page.evaluate(()=>window.dispatchEvent(new Event('beforeprint')));await page.emulateMedia({media:'print'});assert.equal(await page.locator('#statisticsView').isVisible(),true);assert.equal(await page.locator('#registryView').isVisible(),false);
 assert.equal(await page.locator(widget('participation')+' tbody tr').count(),5,'Print includes all performers beyond three-row preview');
 assert.equal(await page.locator(widget('portfolio')+' tbody tr').count(),5,'Print includes all projects beyond three-row preview');
 await page.pdf({path:path.join(out,'native-statistics.pdf'),preferCSSPageSize:true,printBackground:true});await page.emulateMedia({media:null});await page.evaluate(()=>window.dispatchEvent(new Event('afterprint')));
 assert.equal(await page.locator('body').evaluate(e=>e.classList.contains('print-statistics')),false);assert.equal(await previewRows.count(),3,'Normal preview restored after print');
 await page.screenshot({path:path.join(out,'native-statistics.png'),fullPage:true});
 // A real Tauri close request must keep the EXE open when the user continues editing.
 await edit(page);await page.locator(S+'[data-widget-toggle="enterprises"]').uncheck();const beforeClose=await preferences(page);
 await page.evaluate(()=>window.__TAURI__.window.getCurrentWindow().close());await page.waitForSelector(action('leave-continue'));await page.click(action('leave-continue'));await page.waitForSelector(action('leave-continue'),{state:'detached'});
 assert.equal(page.isClosed(),false);assert.equal(await page.locator(action('save-settings')).count(),1,'Continue editing after native close retains the draft');assert.equal(await page.locator(S+'[data-widget-toggle="enterprises"]').isChecked(),false);assert.deepEqual((await preferences(page)).settings,beforeClose.settings,'Native close guard does not silently save the draft');await cancel(page);
 // Follow a real saved project link without replacing the current registry draft.
 await page.click(widget('portfolio')+' [data-action="detail"]');await page.click(action('open-host'));await page.waitForFunction(()=>document.getElementById('registryTab').getAttribute('aria-selected')==='true');
 const summary={source:'packaged EXE / real native IPC / local SQLite',seededProjects:6,seededStages:105,participationMembers:5,percentExample:{share:'1 / 5 * 100 = 20%',completion:'1 / 1 * 100 = 100%'},singleEditor:true,explicitCancelSave:true,reorder:true,dragReorder:true,escapeCancel:true,dragEscapeKeepsDraft:true,leaveContinue:true,nativeCloseContinue:true,boundaryMoveFocus:true,fullWidthCompleteBarTable:true,equalHalfCards:true,filtersRetainedOnRefresh:true,allOrdersNoAutoselection:true,csvFullScopeRows:100,settingsNativeWrite:true,fullTable100Rows:true,invalidExportsRejected:true,statisticsPDFFullScope:true,largeFontFocus:true,projectNavigation:true,tabReopenRetained:true,processRestart:'verified separately by statistics-preferences-native.cjs'};
 fs.writeFileSync(path.join(out,'native-statistics.json'),JSON.stringify(summary,null,2));console.log('PASS native statistics',summary);return summary;
};
