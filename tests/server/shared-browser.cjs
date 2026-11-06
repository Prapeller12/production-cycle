const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require('playwright');
(async()=>{
 const url=process.env.TRUSTED_SERVER_URL;if(!url)throw Error('TRUSTED_SERVER_URL required');
 const executablePath=process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
 const browser=await chromium.launch({headless:true,executablePath});
 const external=[];
 try{
  const context=await browser.newContext({ignoreHTTPSErrors:false});
  await context.route('**/*',r=>{if(new URL(r.request().url()).origin===url)return r.continue();external.push(r.request().url());return r.abort('blockedbyclient')});
  const page=await context.newPage();
  for(const suffix of ['/','/?legacy=1']){
   await page.goto(url+suffix);
   await page.waitForFunction(()=>window.ProductionServer&&window.Production?.backend);
   await page.waitForSelector('#newAuditUserName',{state:'visible'});
   assert.equal(await page.evaluate(()=>fetch('/health').then(r=>r.json()).then(x=>x.mode)),'service');
  }
  assert.deepEqual(external,[]);
  const out=process.env.SERVER_QA_DIR||path.join(__dirname,'../../dist/shared-qa');fs.mkdirSync(out,{recursive:true});
  fs.writeFileSync(path.join(out,'shared-edge.json'),JSON.stringify({httpsCertificateValidation:true,trustScope:'CurrentUser',currentAndLegacy:true,externalNetworkBlocked:true,externalRequests:external},null,2));
  console.log('PASS: Edge accepts CurrentUser certificate trust; current and legacy server pages load with external requests blocked');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
