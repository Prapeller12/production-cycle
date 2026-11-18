'use strict';
// Actual Windows EXE and native IPC; CDP is enabled only in the isolated smoke fixture.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net'),assert=require('node:assert/strict');
const {spawn,spawnSync}=require('node:child_process'),{chromium}=require('playwright');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test,label,timeout=180000){const end=Date.now()+timeout;while(Date.now()<end){const v=await test();if(v)return v;await pause(200)}throw Error('Timeout '+label);}
async function stop(run){if(!run)return;if(run.browser)await run.browser.close().catch(()=>{});spawnSync('taskkill',['/PID',String(run.child.pid),'/T','/F'],{stdio:'ignore',timeout:30000});await wait(()=>run.child.exitCode!==null||run.child.signalCode!==null,'native process exit',30000);}
async function launch(root){
 const port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port));});});
 const report=path.join(root,'temp/self-test.json');fs.rmSync(report,{force:true});
 const child=spawn(path.join(root,'production-cycle.exe'),['--smoke-test'],{cwd:os.tmpdir(),env:{...process.env,PRODUCTION_CYCLE_SMOKE_HOLD:'1',PRODUCTION_CYCLE_SMOKE_BROWSER_ARGS:'--remote-debugging-port='+port},stdio:'ignore'});
 const run={child};let failure;child.once('error',e=>{failure=e;});
 try{
  await wait(async()=>{if(failure)throw failure;if(child.exitCode!==null)throw Error('Native fixture exited before CDP');try{return(await fetch('http://127.0.0.1:'+port+'/json/version',{signal:AbortSignal.timeout(1500)})).ok}catch{return false;}},'native CDP');
  run.browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);run.page=await wait(()=>run.browser.contexts()[0].pages().find(p=>p.url().includes('production.localhost')),'native page');
  await wait(()=>fs.existsSync(report),'native startup report');run.smoke=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(run.smoke.ok,true);assert.equal(run.smoke.frontendReady,true);assert.equal(run.smoke.windowVisible,true);
  await run.page.waitForFunction(()=>window.__smokeDone&&window.__smokeHoldTimer);await run.page.evaluate(()=>clearInterval(window.__smokeHoldTimer));
  await run.page.evaluate(()=>{const modal=document.getElementById('onboardingModal');const dismiss=()=>{if(modal.classList.contains('open'))modal.querySelector('[data-close="onboardingModal"]').click();};new MutationObserver(dismiss).observe(modal,{attributes:true,attributeFilter:['class']});dismiss();});
  return run;
 }catch(e){await stop(run);throw e;}
}
function pristineRoot(repo){assert.equal(process.platform,'win32');const root=path.join(os.tmpdir(),'PC admin '+Date.now());fs.cpSync(path.join(repo,'dist/local/ProductionCycle'),root,{recursive:true});for(const folder of ['data','exports','temp']){fs.rmSync(path.join(root,folder),{recursive:true,force:true});fs.mkdirSync(path.join(root,folder));}return root;}
module.exports={launch,stop,pristineRoot,pause};
