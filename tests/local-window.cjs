// Drive the actual packaged Tauri + Fixed WebView2 window. CDP is enabled only for this CI test.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict'),net=require('node:net'),{spawn,spawnSync}=require('node:child_process'),{chromium}=require('playwright');
const repo=path.resolve(__dirname,'..'),out=path.join(repo,'dist/local-window-qa');fs.mkdirSync(out,{recursive:true});
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const wait=async(f,label,ms=180000)=>{const end=Date.now()+ms;while(Date.now()<end){const value=await f();if(value)return value;await pause(200)}throw Error('Timeout '+label)};
(async()=>{
 const root=path.join(os.tmpdir(),'Local native window '+Date.now());console.log('Preparing pristine native window fixture',root);fs.cpSync(path.join(repo,'dist/local/ProductionCycle'),root,{recursive:true});console.log('Native fixture copied');for(const folder of ['data','exports','temp']){fs.rmSync(path.join(root,folder),{recursive:true,force:true});fs.mkdirSync(path.join(root,folder));}
 console.log('Preparing native debugging port');
 const port=await new Promise(resolve=>{const s=net.createServer();s.listen(0,'127.0.0.1',()=>{const port=s.address().port;s.close(()=>resolve(port))})});
 const report=path.join(root,'temp/self-test.json'),child=spawn(path.join(root,'production-cycle.exe'),['--smoke-test'],{cwd:os.tmpdir(),env:{...process.env,PRODUCTION_CYCLE_SMOKE_HOLD:'1',PRODUCTION_CYCLE_SMOKE_BROWSER_ARGS:'--remote-debugging-port='+port}});
 console.log('Native test EXE started',child.pid,'CDP',port);
 child.on('error',error=>console.error('Native spawn error',error));child.on('exit',(code,signal)=>console.log('Native child exit',code,signal));
 let browser,page;
 try{
  await wait(async()=>{try{const r=await fetch('http://127.0.0.1:'+port+'/json/version',{signal:AbortSignal.timeout(1500)});return r.ok}catch{return false}},'Fixed WebView2 CDP');
  browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);const context=browser.contexts()[0];page=await wait(()=>context.pages().find(p=>p.url().includes('production.localhost')),'native custom protocol page');
  await wait(()=>fs.existsSync(report),'native smoke report');const smoke=JSON.parse(fs.readFileSync(report));assert.equal(smoke.ok,true,JSON.stringify(smoke));assert.equal(smoke.windowVisible,true);assert.equal(smoke.frontendReady,true);
  await page.evaluate(()=>clearInterval(window.__smokeHoldTimer));
  const assets=await page.evaluate(async()=>{await document.fonts.ready;const icon=new Image();icon.src='assets/icons/search.svg';await icon.decode();return {iconWidth:icon.naturalWidth,loaded:[...document.fonts].filter(f=>f.family.includes('Golos')&&f.status==='loaded').length}});assert.ok(assets.iconWidth>0,'Bundled SVG icon did not decode');assert.ok(assets.loaded>0,'Bundled interface font did not load');
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.exposeBinding('__readNativeFile',(_,file)=>{const resolved=fs.realpathSync.native(file),relative=path.relative(fs.realpathSync.native(root),resolved);assert.ok(!relative.startsWith('..')&&!path.isAbsolute(relative));const bytes=fs.readFileSync(resolved);if(/\.xlsx$/i.test(file))assert.equal(require('node:crypto').createHash('sha256').update(bytes).digest('hex'),'1c882f36cdc917d17eccbaf462588c4697a1eb4a4d1bf438f01d967979a2d2c2');return {name:path.basename(file).replace(/ \(\d+\)(?=\.xlsx$)/,''),text:bytes.toString('utf8'),transport:'actual native IPC write'};});
  await page.evaluate(()=>{const original=window.__TAURI__.core.invoke;window.__nativeInvoke=original;const wrapped=async function(command,args){if(command==='print_report'){window.print();return true}const result=await original(command,args);if(command==='save_file'||command==='save_excel_template'){const record=await window.__readNativeFile(result);(window.__buttonDownloads||(window.__buttonDownloads=[])).push(record)}return result};window.__TAURI__.core={...window.__TAURI__.core,invoke:wrapped};if(window.__TAURI__.core.invoke!==wrapped)throw Error('Native core module instrumentation unavailable');window.__buttonAuditFinish=result=>window.__buttonAuditResult=result;});
  await page.evaluate(fs.readFileSync(path.join(repo,'tests/server/button-audit.js'),'utf8'));
  const audit=await page.evaluate(()=>window.__buttonAuditResult);fs.writeFileSync(path.join(out,'native-button-audit.json'),JSON.stringify(audit,null,2));if(audit.error)await page.screenshot({path:path.join(out,'native-button-failure.png'),fullPage:true});assert.equal(audit.error,null,JSON.stringify(audit));assert.deepEqual(audit.coverage.missing,[]);
  console.log('PASS actual native buttons:',audit.coverage.staticButtons,'static;',audit.coverage.dynamicControls,'dynamic;',audit.scenarios.length,'UI scenarios');
  await page.screenshot({path:path.join(out,'native-main.png'),fullPage:true});
  // Reuse the approved production-copy scenario against this native SQLite and IPC, in all roles.
  const arrangeSource=fs.readFileSync(path.join(repo,'tests/server/copy-stage.cjs'),'utf8');
  let arrange=arrangeSource.slice(arrangeSource.indexOf(' const project='),arrangeSource.indexOf('\n(async()=>{const source='));
  arrange=arrange.replace("await c.invoke('network_logout');",'');
  const config=await page.evaluate('(async()=>{const pin="739201",users=await Production.backend.listAuditUsers(),admin=users.find(u=>u.role==="admin"),c={invoke:window.__nativeInvoke};'+arrange+')()');
  await page.click('#networkLogoutBtn');await page.waitForFunction(()=>document.getElementById('networkLoginModal').classList.contains('open'));
  await page.evaluate(config=>{window.__copyConfig=config},config);
  await page.exposeBinding('__copyPointer',(_,selector)=>page.locator(selector).click());
  await page.exposeBinding('__copyFill',(_,id,value)=>page.locator('#'+id).evaluate((e,value)=>{e.value=value;e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}))},value));
  await page.exposeBinding('__copyViewport',(_,w,h)=>page.setViewportSize({width:w,height:h}));
  await page.exposeBinding('__copyCapture',(_,name)=>page.screenshot({path:path.join(out,'copy-'+name+'.png'),fullPage:true}));
  let copySource=fs.readFileSync(path.join(repo,'tests/server/copy-stage-ui.js'),'utf8');
  copySource=copySource.replace("await wait(function(){return open('onboardingModal');},'first rules');await click('[data-close=\"onboardingModal\"]');", "await pause(650);if(open('onboardingModal'))await click('[data-close=\"onboardingModal\"]');");
  const copy=await page.evaluate(copySource);fs.writeFileSync(path.join(out,'native-copy-roles.json'),JSON.stringify(copy,null,2));assert.equal(copy.error,null,JSON.stringify(copy));assert.equal(copy.roles.length,3);
  await page.evaluate(config=>{document.getElementById('networkLoginUser').value=config.users.find(u=>u.role==='admin').id;document.getElementById('networkLoginPin').value='739201';document.getElementById('networkLoginBtn').click()},config);
  await page.waitForFunction(()=>!document.getElementById('networkLoginModal').classList.contains('open'));await page.setViewportSize({width:1440,height:1000});
  // Print content generated by each real control, then inspect actual PDF pages.
  const fixture=require('./server/report-fixture.cjs');
  for(const count of [9,45]){
   await page.evaluate(async fixture=>{document.querySelector('[data-close="onboardingModal"]')?.click();const file=new File([JSON.stringify(fixture)],'report-fixture.json',{type:'application/json'}),input=document.getElementById('jsonFile');Object.defineProperty(input,'files',{configurable:true,value:[file]});input.dispatchEvent(new Event('change',{bubbles:true}));},fixture(count));
   await page.waitForFunction(n=>document.getElementById('orderNo').value==='167'&&document.querySelectorAll('#treeBody tr[data-stage-uid]').length>=n-4,count);
   await page.evaluate(()=>{window.__printSignals=0;window.print=()=>window.__printSignals++;document.querySelector('.more-actions').open=true;document.getElementById('pdfReportBtn').click();});await page.waitForFunction(()=>window.__printSignals===1);
   for(const orientation of ['portrait','landscape']){const style=await page.addStyleTag({content:'@media print{@page{size:A4 '+orientation+';margin:8mm}}'});await page.pdf({path:path.join(out,'native-'+(count===45?'many-':'')+'report-'+orientation+'.pdf'),preferCSSPageSize:true,printBackground:true});await style.evaluate(e=>e.remove());}
  }
  for(const [button,name]of [['printGanttA3Btn','native-gantt-a3'],['printGanttA4Btn','native-gantt-a4'],['printStructureBtn','native-structure']]){
   await page.evaluate(button=>{document.getElementById(button.includes('Gantt')?'ganttTab':'registryTab').click();window.__printSignals=0;document.getElementById(button).click()},button);await page.waitForFunction(()=>window.__printSignals===1);
   await page.pdf({path:path.join(out,name+'.pdf'),preferCSSPageSize:true,printBackground:true});
  }
  await page.evaluate(()=>{window.__TAURI__.core={...window.__TAURI__.core,invoke:window.__nativeInvoke}});
  // The print IPC must also open the actual native print surface, independently of PDF rendering above.
  const printPromise=page.evaluate(()=>window.__TAURI__.core.invoke('print_report'));
  await pause(1500);await page.screenshot({path:path.join(out,'native-print-surface.png')});
  const dismiss=spawnSync('powershell.exe',['-NoProfile','-Command','Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait("{ESC}")'],{encoding:'utf8',timeout:15000});assert.equal(dismiss.status,0,dismiss.stderr);await Promise.race([printPromise,pause(10000).then(()=>{throw Error('Native print IPC did not complete')})]);
  assert.deepEqual(errors,[]);fs.writeFileSync(path.join(out,'native-window-summary.json'),JSON.stringify({architecture:'local EXE / Fixed WebView2 / native IPC / local SQLite',smoke:smoke.checks,buttons:audit.coverage,scenarios:audit.scenarios,nativePrintInvoked:true,errors},null,2));
 }catch(error){if(page)await page.screenshot({path:path.join(out,'failure.png'),fullPage:true}).catch(()=>{});throw error}
 finally{if(page)await page.evaluate(()=>window.__nativeInvoke?.('finish_smoke',{error:null,checks:['UI verification complete']})).catch(()=>{});if(browser)await browser.close().catch(()=>{});spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{stdio:'ignore'});}
})().catch(error=>{console.error(error);process.exitCode=1});
