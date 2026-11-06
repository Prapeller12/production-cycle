const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const out=process.env.PRINT_QA_DIR||path.resolve(__dirname,'../dist/print-qa');fs.mkdirSync(out,{recursive:true});
 const browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH}:{})});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  await page.addInitScript(()=>{localStorage.setItem('production-cycle-onboarding-v1','done');window.print=()=>{window.__printCalled=true}});
  page.on('dialog',dialog=>dialog.accept());
  await page.goto(pathToFileURL(path.resolve(__dirname,'../frontend/index.html')).href);
  await page.locator('.more-actions').evaluate(e=>e.open=true);await page.click('#demoBtn');
  const snapshot=await page.evaluate(()=>{const state=Production.projectFile;return !!state});
  assert.ok(snapshot);
  for(const many of [false,true]){
  if(many){
   const json=await page.evaluate(()=>Production.projectFile.stringify({project:{name:'Многостраничная печать',orderNo:'PRINT',initiator:'Дирекция',executor:'Цех',addressees:'Предприятие',start:'2026-01-01',deadline:'2027-12-31'},stages:Array.from({length:70},(_,i)=>({uid:'p'+(i+1),seq:i+1,sort:i+1,parentUid:i%3===0?null:'p'+(i-i%3+1),title:'Печатный этап '+(i+1),executor:'Цех',addressees:'Предприятие',start:'2026-01-01',deadline:'2027-11-30',status:'work',comment:''})),nextSeq:71,collapsed:{}}));
   await page.locator('#jsonFile').setInputFiles({name:'print.json',mimeType:'application/json',buffer:Buffer.from(json)});await page.waitForFunction(()=>document.querySelector('#orderNo').value==='PRINT');
  }
  for(const mode of ['structure','gantt-a4','gantt-a3']){
   await page.evaluate(()=>window.__printCalled=false);
   if(mode==='structure'){await page.click('#registryTab');await page.click('#printStructureBtn')}
   else{await page.click('[data-view="gantt"]');await page.click(mode==='gantt-a3'?'#printGanttA3Btn':'#printGanttA4Btn')}
   await page.waitForFunction(()=>window.__printCalled);
   await page.pdf({path:path.join(out,(many?'multi-':'')+mode+'.pdf'),preferCSSPageSize:true,printBackground:true});
   await page.evaluate(()=>window.dispatchEvent(new Event('afterprint')));
  }
  }
  console.log('Print PDF artifacts: '+out);
 }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
