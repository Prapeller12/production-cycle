const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require('playwright');
(async()=>{
 const url=process.env.TRUSTED_SERVER_URL;if(!url)throw Error('TRUSTED_SERVER_URL is required');
 const executablePath=process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
 const browser=await chromium.launch({headless:true,executablePath});
 try{
  const context=await browser.newContext({ignoreHTTPSErrors:false}),page=await context.newPage(),external=[];
  await context.route('**/*',r=>{if(new URL(r.request().url()).origin===url)return r.continue();external.push(r.request().url());return r.abort('blockedbyclient')});
  for(const suffix of ['/','/?legacy=1']){
   await page.goto(url+suffix);await page.waitForSelector('#networkLoginModal.open');
   const health=await page.evaluate(()=>new Promise((resolve,reject)=>{const x=new XMLHttpRequest();x.open('GET','/health');x.onload=()=>resolve(JSON.parse(x.responseText));x.onerror=reject;x.send()}));
   assert.equal(health.application,'production-cycle');assert.equal(health.mode,'service');
  }
  assert.deepEqual(external,[]);await context.close();
  const out=process.env.SERVER_QA_DIR||path.join(__dirname,'../../dist/server-qa');fs.mkdirSync(out,{recursive:true});fs.writeFileSync(path.join(out,'trusted-edge.json'),JSON.stringify({httpsCertificateValidation:true,trustScope:'CurrentUser',currentAndLegacy:true,externalRequests:external},null,2));
  console.log('PASS: installed Edge accepts the locally trusted certificate with normal HTTPS validation; current and legacy login pages load without external resources');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
