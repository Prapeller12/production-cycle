'use strict';
// Windows CI acceptance: two complete packaged-EXE processes, genuine profile logins and native preference files.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net'),assert=require('node:assert/strict'),{spawn,spawnSync}=require('node:child_process'),{chromium}=require('playwright');
const {S,action,openStatistics,edit,save,preferences,login,expectSaved}=require('./statistics-native-helpers.cjs');
const repo=path.resolve(__dirname,'..'),out=path.join(repo,'dist/local-window-qa');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const wait=async(test,label,timeout=180000)=>{const until=Date.now()+timeout;while(Date.now()<until){const value=await test();if(value)return value;await pause(200)}throw Error('Timeout '+label)};
async function launch(root){
 const port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port))})});
 const report=path.join(root,'temp/self-test.json');fs.rmSync(report,{force:true});
 const child=spawn(path.join(root,'production-cycle.exe'),['--smoke-test'],{cwd:os.tmpdir(),env:{...process.env,PRODUCTION_CYCLE_SMOKE_HOLD:'1',PRODUCTION_CYCLE_SMOKE_BROWSER_ARGS:'--remote-debugging-port='+port}});
 child.on('error',error=>console.error('Statistics restart EXE launch',error));
 let browser,page;
 try{
  await wait(async()=>{try{return(await fetch('http://127.0.0.1:'+port+'/json/version',{signal:AbortSignal.timeout(1500)})).ok}catch{return false}},'statistics restart CDP');
  browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);
  page=await wait(()=>browser.contexts()[0].pages().find(p=>p.url().includes('production.localhost')),'statistics custom protocol page');
  await wait(()=>fs.existsSync(report),'statistics startup smoke report');const smoke=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(smoke.ok,true,JSON.stringify(smoke));assert.equal(smoke.frontendReady,true);
  await page.waitForFunction(()=>window.__smokeDone&&window.__smokeHoldTimer);await page.evaluate(()=>clearInterval(window.__smokeHoldTimer));
  return {child,browser,page,port,smoke};
 }catch(error){await stop({child,browser,page});throw error}
}
async function stop(run){
 if(!run)return;
 if(run.browser)await run.browser.close().catch(()=>{});
 const result=spawnSync('taskkill',['/PID',String(run.child.pid),'/T','/F'],{encoding:'utf8',windowsHide:true,timeout:30000});
 await wait(()=>run.child.exitCode!==null||run.child.signalCode!==null,'native EXE process exit',30000);
 assert.ok(result.status===0||run.child.exitCode!==null||run.child.signalCode!==null,'The first process must finish before the second starts');
}
async function assertLayout(page,expected){
 const loaded=await expectSaved(page);assert.deepEqual(loaded.settings,expected);
 assert.equal(await page.locator(S+'.pc-widget-grid').getAttribute('data-columns'),String(expected.columns));
 assert.equal(await page.locator('#statisticsMount').getAttribute('data-font'),expected.font);
 assert.deepEqual(await page.locator(S+'.pc-widget').evaluateAll(nodes=>nodes.map(e=>({id:e.dataset.widget,width:e.classList.contains('full')?'full':'half'}))),expected.widgets.filter(w=>w.visible).map(w=>({id:w.id,width:w.width})));
 return loaded;
}
(async()=>{
 assert.equal(process.platform,'win32','This acceptance runner must execute the packaged Windows EXE');fs.mkdirSync(out,{recursive:true});
 const root=path.join(os.tmpdir(),'Statistics process restart '+Date.now());fs.cpSync(path.join(repo,'dist/local/ProductionCycle'),root,{recursive:true});
 for(const folder of ['data','exports','temp']){fs.rmSync(path.join(root,folder),{force:true,recursive:true});fs.mkdirSync(path.join(root,folder));}
 let run,firstPid,secondPid;const errors=[];
 try{
  run=await launch(root);firstPid=run.child.pid;run.page.on('pageerror',error=>errors.push(error.message));
  const users=await run.page.evaluate(()=>Production.backend.listAuditUsers()),a=users.find(u=>u.role==='admin'),b=users.find(u=>u.role==='reviewer');assert.ok(a&&b);
  await openStatistics(run.page);await edit(run.page);
  await run.page.locator(S+'[data-setting="columns"]').selectOption('1');await run.page.locator(S+'[data-setting="font"]').selectOption('large');await run.page.locator(S+'[data-widget-toggle="quality"]').check();await run.page.click(action('move-down','participation'));await save(run.page);
  const savedA=await expectSaved(run.page);assert.equal(savedA.settings.columns,1);assert.equal(savedA.settings.font,'large');
  await login(run.page,b.id);await openStatistics(run.page);
  const freshB=await preferences(run.page);assert.notDeepEqual(freshB.settings,savedA.settings,'Profile B must never receive profile A layout');
  const staleOwnerRejected=await run.page.evaluate(async p=>{try{await window.__TAURI__.core.invoke('statistics_preferences_save',{settings:p.settings,expectedRevision:p.revision,ownerToken:p.ownerToken});return false}catch{return true}},savedA);assert.equal(staleOwnerRejected,true,'Delayed A callback rejected after B login');
  await edit(run.page);await run.page.locator(S+'[data-setting="columns"]').selectOption('2');await run.page.locator(S+'[data-setting="font"]').selectOption('normal');await run.page.locator(S+'[data-widget-toggle="quality"]').uncheck();await run.page.locator(S+'[data-widget-width="participation"]').selectOption('half');await save(run.page);
  const savedB=await expectSaved(run.page);assert.notDeepEqual(savedA.settings,savedB.settings);
  await login(run.page,a.id);await openStatistics(run.page);await assertLayout(run.page,savedA.settings);
  // No localStorage value is needed for recovery: remove only browser preference caches, then terminate the entire EXE tree.
  await run.page.evaluate(()=>{for(let i=localStorage.length-1;i>=0;i--){const key=localStorage.key(i);if(key?.includes('statistics')||key?.includes('dashboard'))localStorage.removeItem(key)}});
  await run.page.screenshot({path:path.join(out,'statistics-profile-a-before-process-exit.png'),fullPage:true});await stop(run);run=null;
  run=await launch(root);secondPid=run.child.pid;assert.notEqual(firstPid,secondPid,'A new OS process must be created');run.page.on('pageerror',error=>errors.push(error.message));
  await openStatistics(run.page);const restoredA=await assertLayout(run.page,savedA.settings);
  assert.equal(restoredA.revision,savedA.revision,'Restart loads the confirmed revision without rewriting it');
  assert.equal(await run.page.locator(S+'[data-filter="orderNo"]').inputValue(),'');assert.equal(await run.page.locator(S+'[data-filter="asOf"]').inputValue(),await run.page.evaluate(()=>PCDomain.today()));
  await run.page.screenshot({path:path.join(out,'statistics-profile-a-after-process-restart.png'),fullPage:true});
  await login(run.page,b.id);await openStatistics(run.page);const restoredB=await assertLayout(run.page,savedB.settings);assert.equal(restoredB.revision,savedB.revision);
  await login(run.page,a.id);await openStatistics(run.page);await assertLayout(run.page,savedA.settings);assert.deepEqual(errors,[]);
  const summary={source:'two actual packaged Windows EXE processes / native IPC / SQLite identity',firstPid,secondPid,firstProcessFullyTerminated:true,browserPreferenceCachesRemoved:true,profileAPersisted:true,profileBPersisted:true,profileIsolation:true,staleOwnerRejected:true,revisionUnchangedAfterRestart:true,newSessionFiltersAllAndToday:true,errors};
  fs.writeFileSync(path.join(out,'statistics-preferences-process-restart.json'),JSON.stringify(summary,null,2));console.log('PASS native statistics process restart',summary);
 }catch(error){if(run?.page)await run.page.screenshot({path:path.join(out,'statistics-preferences-restart-failure.png'),fullPage:true}).catch(()=>{});throw error}
 finally{await stop(run);}
})().catch(error=>{console.error(error);process.exitCode=1});
