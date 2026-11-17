'use strict';
// Invoked by test-statistics-shared-preferences.ps1. No mock IPC, SQLite on SMB,
// alternate business transport, or claim of two physical machines is involved.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {spawn,spawnSync}=require('node:child_process'),{chromium}=require('playwright');
const {login,openStatistics}=require('./statistics-native-helpers.cjs');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const digest=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const readJson=file=>JSON.parse(fs.readFileSync(file,'utf8').replace(/^\uFEFF/,''));
const wait=async(test,label,timeout=180000)=>{const until=Date.now()+timeout;while(Date.now()<until){const value=await test();if(value)return value;await pause(200)}throw Error('Timeout: '+label)};
const plainPath=value=>value.replace(/^\\\\\?\\(?=[A-Za-z]:[\\/])/,'');
const localPath=value=>{assert.equal(typeof value,'string');assert.match(plainPath(value),/^[A-Za-z]:[\\/]/,'Native runtime, profile, SQLite and recovery must be local drive paths');};
const samePath=(a,b)=>path.resolve(plainPath(a)).toLowerCase()===path.resolve(plainPath(b)).toLowerCase();
const within=(base,file)=>{const relative=path.relative(path.resolve(plainPath(base)).toLowerCase(),path.resolve(plainPath(file)).toLowerCase());return !relative.startsWith('..')&&!path.isAbsolute(relative)};
const invoke=(page,command,args)=>page.evaluate(({command,args})=>window.__TAURI__.core.invoke(command,args),{command,args});
const load=page=>invoke(page,'statistics_preferences_load');
const save=(page,loaded,settings)=>invoke(page,'statistics_preferences_save',{settings,expectedRevision:loaded.revision,ownerToken:loaded.ownerToken});
const draft=(page,loaded,settings)=>invoke(page,'statistics_preferences_save_draft',{settings,expectedRevision:loaded.revision,ownerToken:loaded.ownerToken});
const clearDraft=(page,loaded)=>invoke(page,'statistics_preferences_clear_draft',{ownerToken:loaded.ownerToken});
const WIDGETS=['participation','enterprises','portfolio','calendar','executors','risks','statuses','events','quality'];
const settings=(columns,font,theme,reverse=false)=>({version:2,theme,font,horizon:columns===1?30:7,countMode:columns===1?'leaves':'all',columns,widgets:(reverse?[...WIDGETS].reverse():WIDGETS).map((id,i)=>({id,visible:id!=='quality',width:i===0?'full':'half'}))});
const checkLoaded=(value,expected,revision,location='shared')=>{assert.equal(value.state,'loaded');assert.equal(value.location,location);assert.equal(value.revision,revision);assert.deepEqual(value.settings,expected);assert.equal(typeof value.ownerToken,'string');assert.ok(value.ownerToken.length>0);};
const checkSaved=(value,expected,revision)=>{assert.equal(value.state,'saved');assert.equal(value.location,'shared');assert.equal(value.revision,revision);assert.deepEqual(value.settings,expected);};
const cleanError=error=>String(error?.message||error).replace(/739201/g,'[fixture PIN omitted]');

async function freePort(){return new Promise((resolve,reject)=>{const server=net.createServer();server.once('error',reject);server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(error=>error?reject(error):resolve(port))})});}
async function launch(fixture,clientIndex,phase,summary){
 const profile=fixture.clientProfiles[clientIndex],port=await freePort(),out=fixture.outputDirectory;
 const report=path.join(out,phase+'-smoke.json'),startupReport=path.join(out,phase+'-startup.json');
 for(const file of [report,startupReport])fs.rmSync(file,{force:true});
 const child=spawn(path.join(fixture.uncRoot,'production-cycle.exe'),['--smoke-test'],{cwd:os.tmpdir(),env:{...process.env,LOCALAPPDATA:profile,PRODUCTION_CYCLE_SMOKE_REPORT:report,PRODUCTION_CYCLE_STARTUP_REPORT:startupReport,PRODUCTION_CYCLE_SMOKE_HOLD:'1',PRODUCTION_CYCLE_SMOKE_BROWSER_ARGS:'--remote-debugging-port='+port},stdio:'ignore'});
 const run={child,profile,phase};let launchError;
 child.once('error',error=>{launchError=error});
 try{
  await wait(async()=>{if(launchError)throw launchError;if(child.exitCode!==null)throw Error('UNC launcher exited before CDP: '+child.exitCode);try{return(await fetch('http://127.0.0.1:'+port+'/json/version',{signal:AbortSignal.timeout(1500)})).ok}catch{return false}},phase+' WebView2 CDP');
  run.browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);
  run.page=await wait(()=>run.browser.contexts().flatMap(context=>context.pages()).find(page=>page.url().includes('production.localhost')),phase+' native custom protocol page');
  await run.page.waitForFunction(()=>window.__smokeDone&&window.__smokeHoldTimer,undefined,{timeout:180000});
  const result=await run.page.evaluate(()=>{clearInterval(window.__smokeHoldTimer);return window.__smokeDone});
  assert.equal(result.error,null,phase+' packaged native smoke failed');
  run.smoke=await wait(()=>{try{const value=readJson(report);return value.ok&&value.frontendReady&&value.windowVisible?value:null}catch{return null}},phase+' visible native smoke report');
  run.startup=await wait(()=>{try{const value=readJson(startupReport);return value.state==='ready'?value:null}catch{return null}},phase+' local staging report');
  assert.equal(run.smoke.network.enabled,true);assert.equal(run.smoke.network.active,true);assert.ok(run.smoke.checks.length>0);
  assert.equal(run.startup.client.frontendReady,true);assert.equal(run.startup.client.windowVisible,true);
  for(const value of [run.smoke.runtime,run.smoke.profile,run.smoke.database,run.startup.cache,run.startup.recovery]){localPath(value);assert.ok(within(profile,value),phase+' must use its own local client tree');}
  assert.ok(!samePath(run.startup.sharedRoot,run.startup.cache));
  // A newly created identity schedules onboarding after login has already
  // resolved. Dismiss that tutorial through its real close button whenever it
  // appears, so it cannot race the first Statistics tab click for profiles A/B.
  await run.page.evaluate(()=>{
   const modal=document.getElementById('onboardingModal');
   const dismiss=()=>{if(modal.classList.contains('open'))modal.querySelector('[data-close="onboardingModal"]').click();};
   const observer=new MutationObserver(dismiss);observer.observe(modal,{attributes:true,attributeFilter:['class']});dismiss();
  });
  run.pageErrors=[];run.page.on('pageerror',error=>run.pageErrors.push(cleanError(error)));
  assert.ok(Number.isSafeInteger(run.smoke.processId)&&run.smoke.processId>0);
  run.record={phase,client:clientIndex+1,launcherPid:child.pid,nativePid:run.smoke.processId,cache:run.startup.cache,recovery:run.startup.recovery,runtime:run.smoke.runtime,webviewProfile:run.smoke.profile,sqliteDirectory:run.smoke.database,visibleNativeWindow:true,frontendReady:true};summary.launches.push(run.record);
  return run;
 }catch(error){await stop(run);throw error;}
}
async function stop(run){
 if(!run)return;
 if(run.page)await invoke(run.page,'local_logout').catch(()=>{});
 if(run.browser)await run.browser.close().catch(()=>{});
 for(const pid of [...new Set([run.child?.pid,run.smoke?.processId].filter(Boolean))])spawnSync('taskkill',['/PID',String(pid),'/T','/F'],{stdio:'ignore',windowsHide:true,timeout:30000});
 if(run.child?.pid)await wait(()=>run.child.exitCode!==null||run.child.signalCode!==null,'UNC launcher termination',30000);
 if(run.smoke?.processId)await wait(()=>{try{process.kill(run.smoke.processId,0);return false}catch(error){if(error.code==='ESRCH')return true;throw error}},'native client OS process termination',30000);
 if(run.record)run.record.nativeProcessExitConfirmed=true;
}
async function holdSharedFile(file,sharing,out){
 // Windows sharing rules deny replacement/read through the actual UNC file.
 // No ACL, business database, share permissions, or production file is changed.
 const id=crypto.randomUUID(),ready=path.join(out,'fault-'+id+'.ready'),release=path.join(out,'fault-'+id+'.release');
 const command="$ErrorActionPreference='Stop';$stream=$null;try{$stream=[IO.File]::Open($env:PC_STATISTICS_FAULT_FILE,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]$env:PC_STATISTICS_FAULT_SHARING);[IO.File]::WriteAllText($env:PC_STATISTICS_FAULT_READY,'ready');$until=[DateTime]::UtcNow.AddSeconds(90);while(-not [IO.File]::Exists($env:PC_STATISTICS_FAULT_RELEASE)-and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 100}}finally{if($null -ne $stream){$stream.Dispose()}}";
 const child=spawn('powershell.exe',['-NoProfile','-NonInteractive','-Command',command],{env:{...process.env,PC_STATISTICS_FAULT_FILE:file,PC_STATISTICS_FAULT_SHARING:sharing,PC_STATISTICS_FAULT_READY:ready,PC_STATISTICS_FAULT_RELEASE:release},stdio:'ignore',windowsHide:true});
 let spawnError;child.once('error',error=>{spawnError=error});
 const fault={child,ready,release};
 try{await wait(()=>{if(spawnError)throw spawnError;if(child.exitCode!==null)throw Error('Windows SMB sharing-fault helper exited: '+child.exitCode);return fs.existsSync(ready)},'Windows SMB sharing fault '+sharing,30000);return fault;}
 catch(error){await releaseSharedFile(fault);throw error;}
}
async function releaseSharedFile(fault){
 if(!fault)return;
 fs.writeFileSync(fault.release,'release');
 try{await wait(()=>fault.child.exitCode!==null||fault.child.signalCode!==null,'SMB fault handle release',10000);assert.equal(fault.child.exitCode,0,'SMB file handle helper must close normally');}
 finally{if(fault.child.exitCode===null&&fault.child.signalCode===null)fault.child.kill();for(const file of [fault.ready,fault.release])fs.rmSync(file,{force:true});}
}
async function createProfiles(page){
 // The current smoke bootstrap has already created the native administrator.
 // Create the two acceptance identities through the real audited native IPC.
 return page.evaluate(async()=>{
  const invoke=window.__TAURI__.core.invoke,users=await invoke('audit_list_users'),admin=users.find(user=>user.role==='admin');
  if(!admin)throw Error('Missing smoke administrator');
  const input={role:'reviewer',pin:'739201',adminUserId:admin.id,adminPin:'739201'};
  const a=await invoke('audit_create_user',{input:{...input,displayName:'Statistics SMB profile A'}});
  const b=await invoke('audit_create_user',{input:{...input,displayName:'Statistics SMB profile B'}});
  return {a,b};
 });
}

(async()=>{
 assert.equal(process.platform,'win32','This acceptance must run the actual packaged Windows EXE');
 const fixture=readJson(process.argv[2]),out=fixture.outputDirectory;fs.mkdirSync(out,{recursive:true});
 assert.match(fixture.uncRoot,/^\\\\127\.0\.0\.1\\/,'Only an isolated loopback SMB fixture is permitted');
 assert.equal(fixture.clientProfiles.length,2);assert.ok(!samePath(...fixture.clientProfiles));
 const summary={ok:false,source:'new packaged Windows EXE / native IPC / real loopback SMB UNC',environment:{os:fixture.hostOs,version:fixture.sourceVersion,zipSha256:fixture.zipSha256,smbConnections:fixture.smbConnections,physicalCiHosts:1,independentClientCaches:2},launches:[],checks:[],phase:'initialization'};
 const resultFile=path.join(out,'statistics-shared-preferences.json');let first,second,fault;
 const check=(name,details={})=>summary.checks.push({name,...details});
 const phase=name=>{summary.phase=name;fs.writeFileSync(resultFile,JSON.stringify(summary,null,2));console.log('Statistics SMB acceptance:',name)};
 try{
  for(const profile of fixture.clientProfiles)assert.equal(fs.existsSync(path.join(profile,'ProductionCycleNetwork')),false,'Each independent client starts without a cache or recovery files');
  const prefsRoot=path.join(fixture.uncRoot,'shared/preferences');
  phase('first native process creates two profiles and saves profile A');
  first=await launch(fixture,0,'client-a-first',summary);const identities=await createProfiles(first.page);
  for(const user of Object.values(identities))assert.match(user.keyFingerprint,/^[0-9A-Fa-f]{16}$/);
  assert.notEqual(identities.a.keyFingerprint,identities.b.keyFingerprint);
  const fileA=path.join(prefsRoot,identities.a.keyFingerprint.toUpperCase(),'statistics.json'),fileB=path.join(prefsRoot,identities.b.keyFingerprint.toUpperCase(),'statistics.json');
  const layoutA=settings(1,'large','dark',true),layoutB=settings(2,'normal','light'),localDraft=settings(2,'large','system');
  await login(first.page,identities.a.id);let loaded=await load(first.page);assert.equal(loaded.revision,0);assert.equal(loaded.settings,null);assert.equal(loaded.location,'shared');
  checkSaved(await save(first.page,loaded,layoutA),layoutA,1);loaded=await load(first.page);checkLoaded(loaded,layoutA,1);
  assert.deepEqual(readJson(fileA).settings,layoutA);const workspace=readJson(path.join(prefsRoot,'workspace.json')).workspaceId;assert.match(workspace,/^[0-9a-f-]{36}$/);
  assert.equal((await draft(first.page,loaded,localDraft)).state,'draft_saved');assert.equal(readJson(fileA).revision,1);
  const firstCache=first.startup.cache,firstRecovery=first.startup.recovery,firstNativePid=first.smoke.processId;
  await openStatistics(first.page);await first.page.screenshot({path:path.join(out,'shared-profile-a-first-process.png'),fullPage:true});
  assert.deepEqual(first.pageErrors,[]);await stop(first);first=null;
  check('profile A native IPC creation and authoritative SMB save',{revision:1,localDraftKeptSeparate:true});

  phase('independent clean client loads A and saves isolated B');
  assert.equal(fs.existsSync(path.join(fixture.clientProfiles[1],'ProductionCycleNetwork')),false,'Second client must still have no local preference cache');
  second=await launch(fixture,1,'client-b-clean',summary);await login(second.page,identities.a.id);loaded=await load(second.page);checkLoaded(loaded,layoutA,1);assert.equal(loaded.draft,null,'Local draft must not cross independent client caches');
  assert.ok(!samePath(firstCache,second.startup.cache));assert.ok(!samePath(firstRecovery,second.startup.recovery));
  await login(second.page,identities.b.id);let bLoaded=await load(second.page);assert.equal(bLoaded.settings,null);assert.equal(bLoaded.revision,0);assert.equal(bLoaded.draft,null);
  checkSaved(await save(second.page,bLoaded,layoutB),layoutB,1);bLoaded=await load(second.page);checkLoaded(bLoaded,layoutB,1);assert.deepEqual(readJson(fileB).settings,layoutB);assert.deepEqual(readJson(fileA).settings,layoutA);
  await login(second.page,identities.a.id);checkLoaded(await load(second.page),layoutA,1);assert.deepEqual(second.pageErrors,[]);await stop(second);second=null;
  check('independent clean client loads shared A without its local draft');check('profile B isolated from A on shared storage',{profileARevision:1,profileBRevision:1});

  phase('complete process restart and local cache repair preserve shared A and local draft');
  // Corrupt only disposable local assets. The new ZIP and shared data stay intact.
  fs.appendFileSync(path.join(firstCache,'frontend/index.html'),'\n<!-- CI damaged local cache -->');
  first=await launch(fixture,0,'client-a-cache-repaired',summary);assert.notEqual(first.smoke.processId,firstNativePid,'Restart must create a new native OS process');assert.ok(!samePath(first.startup.cache,firstCache),'Damaged cache must be rebuilt in another local directory');assert.ok(samePath(first.startup.recovery,firstRecovery));
  assert.equal(readJson(path.join(prefsRoot,'workspace.json')).workspaceId,workspace);
  await login(first.page,identities.a.id);loaded=await load(first.page);checkLoaded(loaded,layoutA,1);assert.deepEqual(loaded.draft.settings,localDraft);assert.equal(loaded.draft.expectedRevision,1);await clearDraft(first.page,loaded);assert.equal((await load(first.page)).draft,null);
  await login(first.page,identities.b.id);checkLoaded(await load(first.page),layoutB,1);await login(first.page,identities.a.id);
  check('new native process and repaired local asset cache recover confirmed A and local draft',{revisionUnchanged:1,sharedWorkspaceUnchanged:true,stableRecoveryDirectory:true});

  phase('two admitted sessions for A reject a stale CAS revision');
  second=await launch(fixture,1,'client-b-concurrent',summary);await login(second.page,identities.a.id);
  const concurrentStatus=await invoke(second.page,'local_status');assert.equal(concurrentStatus.active,true);assert.equal(concurrentStatus.occupied,2,'Both native sessions must be admitted concurrently');
  const staleFirst=await load(first.page),staleSecond=await load(second.page);checkLoaded(staleFirst,layoutA,1);checkLoaded(staleSecond,layoutA,1);assert.notEqual(staleFirst.ownerToken,staleSecond.ownerToken,'Each native session owns its own callback token');
  const winner=settings(2,'normal','system',true),loser=settings(1,'normal','light');
  checkSaved(await save(first.page,staleFirst,winner),winner,2);const winnerHash=digest(fs.readFileSync(fileA));
  const conflict=await save(second.page,staleSecond,loser);assert.equal(conflict.state,'conflict');assert.equal(conflict.location,'shared');assert.equal(conflict.revision,2);assert.deepEqual(conflict.settings,winner);assert.equal(digest(fs.readFileSync(fileA)),winnerHash,'Rejected stale CAS must not alter shared bytes');
  loaded=await load(second.page);checkLoaded(loaded,winner,2);checkSaved(await save(second.page,loaded,loser),loser,3);checkLoaded(await load(first.page),loser,3);checkLoaded(await load(second.page),loser,3);
  check('two real concurrent native sessions for A compare authoritative SMB revision',{concurrentAdmittedSessions:2,firstRevision:1,winnerRevision:2,staleSave:'conflict',explicitRetryRevision:3,losingSaveDidNotAlterSharedBytes:true});

  phase('SMB replacement failure preserves confirmed revision and a local unsaved draft');
  loaded=await load(first.page);const failedLayout=settings(1,'large','system',true),confirmedHash=digest(fs.readFileSync(fileA));
  assert.equal((await draft(first.page,loaded,failedLayout)).state,'draft_saved');
  fault=await holdSharedFile(fileA,'Read',out);
  const failedSave=await first.page.evaluate(async args=>{try{return {rejected:false,result:await window.__TAURI__.core.invoke('statistics_preferences_save',args)}}catch(error){return {rejected:true,error:String(error)}}},{settings:failedLayout,expectedRevision:loaded.revision,ownerToken:loaded.ownerToken});
  assert.equal(failedSave.rejected,true,'Actual SMB sharing denial must reject save instead of reporting success');assert.ok(failedSave.error.length>0);assert.equal(digest(fs.readFileSync(fileA)),confirmedHash);checkLoaded(await load(second.page),loser,3);
  await releaseSharedFile(fault);fault=null;
  loaded=await load(first.page);checkLoaded(loaded,loser,3);assert.deepEqual(loaded.draft.settings,failedLayout);assert.equal(loaded.draft.expectedRevision,3);
  check('real SMB atomic replacement failure is reported and preserves shared revision',{fault:'Windows UNC file handle denies write/delete sharing',confirmedRevision:3,authoritativeBytesUnchanged:true,localDraftRetained:true});

  phase('SMB read failure uses confirmed local cache and recovers after handle release');
  fault=await holdSharedFile(fileA,'None',out);loaded=await load(first.page);checkLoaded(loaded,loser,3,'local');assert.ok(loaded.warning,'Local fallback must visibly report an SMB warning');assert.deepEqual(loaded.draft.settings,failedLayout);
  await releaseSharedFile(fault);fault=null;assert.equal(digest(fs.readFileSync(fileA)),confirmedHash);
  loaded=await load(first.page);checkLoaded(loaded,loser,3);checkSaved(await save(first.page,loaded,failedLayout),failedLayout,4);loaded=await load(first.page);await clearDraft(first.page,loaded);checkLoaded(await load(second.page),failedLayout,4);checkLoaded(await load(first.page),failedLayout,4);assert.equal((await load(first.page)).draft,null);
  await login(second.page,identities.b.id);checkLoaded(await load(second.page),layoutB,1);assert.deepEqual(readJson(fileB).settings,layoutB);
  check('real SMB read sharing failure returns local cache with warning and later recovers',{fallbackRevision:3,resumedSharedRevision:4,profileBRevision:1,draftClearedAfterConfirmedSave:true});
  assert.deepEqual(first.pageErrors,[]);assert.deepEqual(second.pageErrors,[]);await stop(first);first=null;await stop(second);second=null;
  await wait(()=>fs.readdirSync(path.join(fixture.uncRoot,'shared/sessions')).filter(name=>/^session-.*\.json$/.test(name)).length===0,'native sessions released',30000);check('all acceptance native sessions released');
  summary.ok=true;summary.phase='complete';
 }catch(error){summary.error=cleanError(error);if(first?.page)await first.page.screenshot({path:path.join(out,'shared-preferences-failure.png'),fullPage:true}).catch(()=>{});throw error;}
 finally{try{await releaseSharedFile(fault)}finally{await stop(first);await stop(second);fs.writeFileSync(resultFile,JSON.stringify(summary,null,2));}}
 console.log('PASS actual Windows SMB Statistics preferences:',summary.checks.length,'checks');
})().catch(error=>{console.error('FAIL Statistics SMB preferences:',cleanError(error));process.exitCode=1;});
