const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require('playwright');
(async()=>{
 const url=process.env.TRUSTED_SERVER_URL;if(!url)throw Error('TRUSTED_SERVER_URL required');
 const executablePath=process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
 const browser=await chromium.launch({headless:true,executablePath});
 const external=[];
 try{
  async function session(origin){
   const context=await browser.newContext({ignoreHTTPSErrors:false,viewport:{width:1440,height:1000}});
   await context.addInitScript(()=>localStorage.setItem('production-cycle-onboarding-v1','done'));
   await context.route('**/*',r=>{if(new URL(r.request().url()).origin===origin)return r.continue();external.push(r.request().url());return r.abort('blockedbyclient')});
   const page=await context.newPage();page.setDefaultTimeout(20000);page.setDefaultNavigationTimeout(20000);
   return {context,page};
  }
  // Initial administrator creation is intentionally allowed only from the server itself.
  const local='https://localhost:'+new URL(url).port;
  console.log('ACCEPTANCE: create the first administrator through localhost');
  const setup=await session(local);
  await setup.page.goto(local+'/');
  await setup.page.waitForSelector('#newAuditUserName',{state:'visible'});
  await setup.page.fill('#newAuditUserName','Администратор проверки общей папки');
  await setup.page.fill('#newAuditUserPin','739201');await setup.page.fill('#newAuditUserPinConfirm','739201');
  await setup.page.click('#createAuditUserBtn');
  await setup.page.waitForFunction(()=>Production.backend.networkStatus().then(s=>s.active&&s.userId));
  await setup.page.waitForFunction(()=>!document.querySelector('#createAuditUserBtn').disabled&&document.querySelector('#auditUserError .ok'));
  await setup.page.waitForTimeout(500);if(await setup.page.locator('#onboardingModal.open').count())await setup.page.locator('[data-close="onboardingModal"]').click();
  await setup.page.click('#networkLogoutBtn');await setup.page.waitForSelector('#networkLoginModal.open');
  await setup.context.close();
  for(const suffix of ['/','/?legacy=1']){
   console.log('ACCEPTANCE: sign in through the network URL '+suffix);
   const {context,page}=await session(url);
   await page.goto(url+suffix);
   await page.waitForFunction(()=>window.ProductionServer&&window.Production?.backend);
   await page.waitForSelector('#networkLoginModal.open');
   await page.fill('#networkLoginPin','739201');await page.click('#networkLoginBtn');
   await page.waitForFunction(()=>Production.backend.networkStatus().then(s=>s.active&&s.userId));
   await page.waitForSelector('#networkLoginModal.open',{state:'hidden'});
   await page.waitForFunction(()=>!document.querySelector('#networkLoginBtn').disabled);
   assert.equal(await page.evaluate(()=>fetch('/health').then(r=>r.json()).then(x=>x.mode)),'service');
   await page.waitForTimeout(500);if(await page.locator('#onboardingModal.open').count())await page.locator('[data-close="onboardingModal"]').click();
   await page.click('#networkLogoutBtn');await page.waitForSelector('#networkLoginModal.open');
   await context.close();
  }
  assert.deepEqual(external,[]);
  const out=process.env.SERVER_QA_DIR||path.join(__dirname,'../../dist/shared-qa');fs.mkdirSync(out,{recursive:true});
  fs.writeFileSync(path.join(out,'shared-edge.json'),JSON.stringify({httpsCertificateValidation:true,trustScope:'CurrentUser',currentAndLegacy:true,localAdministratorCreated:true,remoteLoginAndLogout:true,externalNetworkBlocked:true,externalRequests:external},null,2));
  console.log('PASS: Edge accepts CurrentUser certificate trust; current and legacy server pages load with external requests blocked');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
