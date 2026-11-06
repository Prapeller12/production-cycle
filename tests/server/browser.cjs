const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{startServer,repo}=require('./helpers.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const server=await startServer();
 const browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH}:{})});
 const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/server-qa');fs.mkdirSync(out,{recursive:true});
 try{
  for(const legacy of [false,true]){
   const context=await browser.newContext({ignoreHTTPSErrors:true,viewport:{width:1440,height:1000}}),page=await context.newPage(),errors=[];
   page.on('pageerror',e=>errors.push(e.message));page.on('dialog',d=>d.accept());
   await page.addInitScript(()=>localStorage.setItem('production-cycle-onboarding-v1','done'));
   await page.goto(server.url+(legacy?'/?legacy=1':'/'));
   await page.waitForFunction(()=>window.ProductionServer&&window.Production?.backend);
   await page.evaluate(()=>{window.__smokeDone=null;window.__browserExports=[];window.__browserTestInvoke=async(command,args)=>{if(command==='finish_smoke'){window.__smokeDone=args;return false}if(command==='save_file'){window.__browserExports.push(args);return args.name}throw Error('Unsupported test hook '+command)}});
   let smoke=fs.readFileSync(path.join(repo,'tests/network-native-smoke.js'),'utf8').replaceAll('window.__TAURI__.core.invoke','window.__browserTestInvoke');
   await page.evaluate(smoke);
   await page.waitForFunction(()=>window.__smokeDone!==null,{},{timeout:120000});
   const result=await page.evaluate(()=>window.__smokeDone);assert.equal(result.error,null,JSON.stringify(result));assert.equal(errors.length,0,errors.join('; '));
   fs.writeFileSync(path.join(out,legacy?'legacy-browser.json':'edge-browser.json'),JSON.stringify(result,null,2));
   await page.click('#loadJsonBtn');await page.locator('[data-open-project-id]').first().click();
   await page.locator('#treeBody [data-act="copy"]').first().click();assert.equal(await page.locator('#stageModalTitle').innerText(),'Копирование этапа');assert.equal(await page.inputValue('#stageStatus'),'new');await page.click('#saveStageBtn');
   await page.screenshot({path:path.join(out,legacy?'legacy-interface.png':'edge-interface.png'),fullPage:true});
   await page.evaluate(()=>{window.print=()=>{window.__printed=true}});
   for(const mode of ['structure','gantt-a4','gantt-a3']){
    await page.evaluate(()=>window.__printed=false);
    if(mode==='structure')await page.click('#printStructureBtn');else{await page.click('#ganttTab');await page.click(mode==='gantt-a3'?'#printGanttA3Btn':'#printGanttA4Btn')}
    await page.waitForFunction(()=>window.__printed);
    await page.pdf({path:path.join(out,(legacy?'legacy-':'edge-')+mode+'.pdf'),preferCSSPageSize:true,printBackground:true});
    await page.evaluate(()=>window.dispatchEvent(new Event('afterprint')));
   }
   await page.click('#networkLogoutBtn');await page.waitForSelector('#networkLoginModal.open');await context.close();
   console.log((legacy?'IE11 ES5 bundle in Chromium':'Edge/current bundle')+': real server and UI functional scenarios passed');
  }
 }finally{await browser.close();await server.stop()}
})().catch(e=>{console.error(e);process.exitCode=1});
