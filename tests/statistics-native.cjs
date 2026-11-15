'use strict';
// Invoked by local-window.cjs against the real packaged EXE, SQLite and native IPC.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
module.exports=async(page,out,root)=>{
 const scope='#statisticsMount ';
 assert.equal(await page.locator('#ganttTab').evaluate(e=>e.nextElementSibling.id),'statisticsTab');
 await page.click('#statisticsTab');await page.waitForSelector(scope+'[data-widget="participation"]');
 const expected=await page.evaluate(async()=>{
  const summaries=await Production.backend.listProjects(),projects=[];
  for(const p of summaries)projects.push(await Production.backend.loadById(p.projectId));
  const data=PCDomain.normalize({schemaVersion:1,projects}),orderNo=document.querySelector('[data-control="participationOrder"]').value;
  return {projects:summaries.length,total:projects.reduce((n,p)=>n+p.stages.length,0),participation:PCDomain.participation(data,{}, {},orderNo)};
 });
 assert.equal(await page.locator(scope+'[data-widget]').count(),9);
 const rows=await page.locator(scope+'.pc-participation-table tbody tr').evaluateAll(rows=>rows.map(r=>[...r.cells].map(c=>c.textContent.trim())));
 assert.equal(rows.length,expected.participation.members.length);
 for(const [i,m]of expected.participation.members.entries()){
  assert.equal(rows[i][0],m.executor);assert.equal(Number(rows[i][2]),m.assigned);assert.equal(Number(rows[i][3]),m.done);
  assert.ok(rows[i][1].includes(m.sharePercent.toLocaleString('ru-RU',{maximumFractionDigits:1})+'%'));
  assert.equal(rows[i][4],m.donePercent.toLocaleString('ru-RU',{maximumFractionDigits:1})+'%');
 }
 await page.evaluate(()=>window.__buttonDownloads=[]);await page.click(scope+'[data-action="export-csv"]');
 await page.waitForFunction(()=>window.__buttonDownloads.some(x=>x.name.endsWith('.csv')));
 const csv=await page.evaluate(()=>window.__buttonDownloads.find(x=>x.name.endsWith('.csv')));
 assert.ok(csv.text.startsWith('\uFEFF'));assert.equal(csv.text.split('\uFEFF').length-1,1,'Exactly one UTF-8 BOM');assert.ok(csv.text.length>30);
 await page.click(scope+'.pc-page-actions [data-action="settings"]');
 await page.locator(scope+'[data-widget-toggle="quality"]').uncheck();
 await page.click(scope+'[data-action="export-settings"]');
 await page.waitForFunction(()=>window.__buttonDownloads.some(x=>x.name==='dashboard-settings.json'));
 const settings=await page.evaluate(()=>JSON.parse(window.__buttonDownloads.find(x=>x.name==='dashboard-settings.json').text.replace(/^\uFEFF/,'')));
 assert.equal(settings.widgets.find(x=>x.id==='quality').visible,false);
 await page.click(scope+'[data-action="close-dialog"]');assert.equal(await page.locator(scope+'[data-widget="quality"]').count(),0);
 await page.click('#registryTab');await page.click('#statisticsTab');await page.waitForSelector(scope+'[data-widget="participation"]');
 assert.equal(await page.locator(scope+'[data-widget="quality"]').count(),0,'Settings retained after reopening');
 await page.click(scope+'.pc-page-actions [data-action="settings"]');await page.locator(scope+'[data-widget-toggle="quality"]').check();await page.click(scope+'[data-action="close-dialog"]');
 // Real native command validation must reject unrelated files and malformed settings.
 const rejected=await page.evaluate(async()=>{const invoke=window.__nativeInvoke;const result=[];for(const [name,text]of [['../escape.csv','x'],['bad.exe','x'],['bad.json','{}']]){try{await invoke('save_statistics_file',{name,text});result.push(false)}catch{result.push(true)}}return result});assert.deepEqual(rejected,[true,true,true]);
 await page.evaluate(()=>{window.__printSignals=0;window.print=()=>window.__printSignals++});
 await page.click(scope+'[data-action="print"]');await page.waitForFunction(()=>window.__printSignals===1);
 await page.emulateMedia({media:'print'});assert.equal(await page.locator('#statisticsView').isVisible(),true);assert.equal(await page.locator('#registryView').isVisible(),false);
 await page.pdf({path:path.join(out,'native-statistics.pdf'),preferCSSPageSize:true,printBackground:true});await page.emulateMedia({media:'screen'});
 await page.evaluate(()=>window.dispatchEvent(new Event('afterprint')));
 assert.equal(await page.locator('body').evaluate(e=>e.classList.contains('print-statistics')),false);
 await page.screenshot({path:path.join(out,'native-statistics.png'),fullPage:true});
 // Follow a real dashboard project link; use the original draft confirmation.
 await page.click(scope+'[data-widget="portfolio"] [data-action="detail"]');await page.click(scope+'[data-action="open-host"]');
 await page.waitForFunction(()=>document.getElementById('registryTab').getAttribute('aria-selected')==='true');
 const summary={source:'packaged EXE / real native IPC / local SQLite',projects:expected.projects,stages:expected.total,participationMembers:rows.length,csvNativeWrite:true,settingsNativeWrite:true,settingsRetained:true,invalidExportsRejected:true,statisticsPDF:true,projectNavigation:true};
 fs.writeFileSync(path.join(out,'native-statistics.json'),JSON.stringify(summary,null,2));console.log('PASS native statistics',summary);return summary;
};
