const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{startServer,repo}=require('./helpers.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/button-qa');fs.mkdirSync(out,{recursive:true});
 const browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH}:{})});
 try{
  for(const legacy of [false,true]){
   const server=await startServer(),context=await browser.newContext({ignoreHTTPSErrors:true,acceptDownloads:true,viewport:{width:1440,height:1000}}),page=await context.newPage(),errors=[],external=[],downloadErrors=[];
   try{
    await context.route('**/*',route=>{const u=new URL(route.request().url());if(u.origin===server.url||u.protocol==='blob:')return route.continue();external.push(u.href);return route.abort('blockedbyclient')});
    page.on('pageerror',error=>errors.push(error.message));page.on('dialog',dialog=>dialog.accept());
    page.on('download',async download=>{try{const target=path.join(out,(legacy?'legacy-':'edge-')+path.basename(download.suggestedFilename()));await download.saveAs(target);const bytes=fs.readFileSync(target);if(/\.xlsx$/i.test(download.suggestedFilename())){const template=JSON.parse(fs.readFileSync(path.join(repo,'server/resources/project-import-template.json')));assert.equal(require('node:crypto').createHash('sha256').update(bytes).digest('hex'),template.sha256)}else assert.equal(bytes.subarray(0,3).toString('hex'),'efbbbf');const item={name:download.suggestedFilename(),text:bytes.toString('utf8'),transport:'Edge actual browser download'};await page.evaluate(item=>window.__buttonDownloads.push(item),item);}catch(error){downloadErrors.push(String(error));}});
    await page.addInitScript(()=>localStorage.setItem('production-cycle-onboarding-v1','done'));
    await page.goto(server.url+(legacy?'/?legacy=1':'/'));
    await page.waitForFunction(()=>window.ProductionServer&&window.Production?.backend);
    await page.evaluate(fs.readFileSync(path.join(repo,'tests/server/button-trace.js'),'utf8'));
    await page.evaluate(()=>{window.__smokeDone=null;window.__browserTestInvoke=async(command,args)=>{if(command==='finish_smoke'){window.__smokeDone=args;return false}if(command==='save_file')return args.name;throw Error('Unexpected test hook '+command)};window.__buttonAuditFinish=result=>window.__buttonAuditResult=result;});
    await page.evaluate(fs.readFileSync(path.join(repo,'tests/network-native-smoke.js'),'utf8').replaceAll('window.__TAURI__.core.invoke','window.__browserTestInvoke'));
    await page.evaluate(fs.readFileSync(path.join(repo,'tests/server/button-audit.js'),'utf8'));
    const result=await page.evaluate(()=>window.__buttonAuditResult);fs.writeFileSync(path.join(out,legacy?'legacy-button-audit.json':'edge-button-audit.json'),JSON.stringify({...result,offline:{externalRequests:external,externalNetworkBlocked:true}},null,2));
    if(result.error)await page.screenshot({path:path.join(out,legacy?'legacy-button-failure.png':'edge-button-failure.png'),fullPage:true});
    assert.equal(result.error,null,JSON.stringify(result));assert.deepEqual(result.coverage.missing,[]);assert.ok(result.scenarios.length>=16);assert.deepEqual(errors,[]);assert.deepEqual(downloadErrors,[]);assert.deepEqual(external,[]);
    console.log('PASS '+(legacy?'ES5 in Edge':'Edge')+' button audit:',result.coverage.staticButtons,'static buttons;',result.coverage.dynamicControls,'dynamic controls;',result.scenarios.length,'UI scenarios');
   }finally{await context.close();await server.stop()}
  }
 }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
