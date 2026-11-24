// Exercise the shipped EXE and its real unlock dialog using disposable projects and keys.
const {chromium}=require('playwright'), fs=require('node:fs'), path=require('node:path'), os=require('node:os');
const {spawn,execFileSync}=require('node:child_process'), net=require('node:net'), assert=require('node:assert/strict');
const pause=n=>new Promise(r=>setTimeout(r,n));
async function until(f,title,ms=150000){const end=Date.now()+ms;while(Date.now()<end){const x=await f();if(x)return x;await pause(200)}throw Error('Timeout: '+title)}
async function port(){const server=net.createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const p=server.address().port;await new Promise(r=>server.close(r));return p}
const [root,mode,work]=process.argv.slice(2),isShared=mode==='shared',pin='729418',pass='QAFileTransfer729418';
const run=async()=>{
 const p=await port();const secret='AQPrivateStore'+Math.random().toString(36).slice(2);
 const report=path.join(work,mode+'-startup.json');
 const child=spawn(path.join(root,'production-cycle.exe'),isShared?[]:['--offline-workstation'],{cwd:os.tmpdir(),env:{...process.env,PRODUCTION_CYCLE_LAUNCH_STATUS:report,PRODUCTION_CYCLE_STARTUP_REPORT:report,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port='+p}});
 const unlock=spawn('powershell.exe',['-NoProfile','-STA','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'unlock-test-window.ps1')],{env:{...process.env,PC_TEST_WINDOW_PHRASE:secret},stdio:'inherit'});
 const unlockExit=new Promise(r=>unlock.on('exit',r));
 let browser,page;
 try{
  assert.equal(await unlockExit,0,'unlock dialog');console.log('Native unlock submitted: '+mode);
  await until(async()=>{try{return (await fetch('http://127.0.0.1:'+p+'/json/version',{signal:AbortSignal.timeout(1000)})).ok}catch{return false}},'WebView2 port');console.log('Packaged WebView connected: '+mode);
  browser=await chromium.connectOverCDP('http://127.0.0.1:'+p);
  page=await until(()=>browser.contexts()[0]?.pages().find(x=>x.url().includes('production.localhost')||x.url().includes('production://localhost')),'native page');
  await page.waitForFunction(()=>window.ProductionDbExchange && window.__TAURI__?.core?.invoke);
  assert.equal(await page.evaluate(()=>Boolean(window.ProductionShared)),isShared);
  await page.evaluate(async({pin,isShared})=>{
    const call=(n,a)=>window.__TAURI__.core.invoke(n,a);
    await call('local_login',{userId:null,pin:''});
    const user=await call('audit_create_user',{input:{displayName:'Испытатель',pin,isAdmin:true}});
    await call('local_logout');await call('local_login',{userId:user.id,pin});
    if(!isShared){
      await call('audit_save_snapshot',{snapshot:{project:{name:'Проверка обмена',orderNo:'005',initiator:'Цех',executor:'ОТК',enterprise:'Завод',start:'2026-12-01',deadline:'2026-12-31'},stages:[{uid:'ui-stage',seq:1,sort:1,parentUid:null,title:'Контроль',executor:'Сборка',addressees:'ОТК',start:'2026-12-01',deadline:'2026-12-15',status:'new',comment:''}],nextSeq:2},userId:user.id,pin,comment:'Испытание',evidence:null});
    }
  },{pin,isShared});
  await page.reload();await page.waitForFunction(()=>window.ProductionDbExchange && document.querySelector('#importBtn'));
  // Dismiss the optional first-login guide if opened. Use the public menu event.
  await page.evaluate(()=>{document.querySelector('#onboardingModal.open [data-close="onboardingModal"]')?.click();document.querySelector('details.more-actions').open=true;document.querySelector('#importBtn').click()});
  await page.locator('#exchangeDbModal.open').waitFor();
  if(!isShared){
    await page.locator('#xdbProjects input').first().check();
    await page.locator('#xdbPass').fill(pass);await page.locator('#xdbPin').fill(pin);await page.locator('#xdbGo').click();
    await page.waitForFunction(()=>document.querySelector('#xdbStatus').textContent.includes('.pcdb'));
    const f=(await page.locator('#xdbStatus').innerText()).trim().split('\n').at(-1);assert.ok(fs.existsSync(f));fs.copyFileSync(f,path.join(work,'roundtrip.pcdb'));
    assert.notEqual(fs.readFileSync(f).subarray(0,16).toString(),'SQLite format 3\0');
  }else{
    await page.locator('#xdbImportTab').click();await page.locator('#xdbFile').setInputFiles(path.join(work,'roundtrip.pcdb'));
    await page.locator('#xdbPass').fill(pass);await page.locator('#xdbInspect').click();
    await page.locator('#xdbMapping select').first().selectOption('0');await page.locator('#xdbMapping input[type=checkbox]').check();
    await page.locator('#xdbPlan').click();await page.waitForFunction(()=>!document.querySelector('#xdbGo').disabled);
    await page.locator('#xdbPin').fill(pin);await page.locator('#xdbComment').fill('Подтверждаю тестовый перенос');await page.locator('#xdbGo').click();
    await page.waitForFunction(()=>document.querySelector('#xdbStatus').textContent.includes('Приём записан'));
    const projects=await page.evaluate(()=>window.__TAURI__.core.invoke('production_list_projects'));assert.equal(projects.length,1);assert.equal(projects[0].stageCount,1);
    assert.notEqual(fs.readFileSync(path.join(root,'shared/database/production-cycle.db')).subarray(0,16).toString(),'SQLite format 3\0');
  }
  console.log('Packaged Windows UI passed: '+mode);
 }catch(e){
  if(fs.existsSync(report))console.error(fs.readFileSync(report,'utf8'));
  if(page)console.error(await page.evaluate(()=>({status:document.querySelector('#xdbStatus')?.textContent,modal:[...document.querySelectorAll('.modal.open')].map(x=>x.id),page:location.href})).catch(()=>null));throw e;
 }finally{
  if(page){try{await page.evaluate(()=>window.__TAURI__.core.invoke('local_logout'))}catch{}}
  if(browser)await browser.close();
  // Runner is single-use and contains only the application processes started by this test.
  try{execFileSync('taskkill',['/T','/F','/IM','production-cycle.exe'],{stdio:'ignore'})}catch{}
  if(!child.killed)child.kill();if(!unlock.killed)unlock.kill()
 }
};run().catch(e=>{console.error(e);process.exitCode=1});
