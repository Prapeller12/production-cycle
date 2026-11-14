// Reproduce both reported faults, then exercise the corrected package with the same faults.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),http=require('node:http'),assert=require('node:assert/strict'),crypto=require('node:crypto'),{spawn}=require('node:child_process');
const {startServer,repo}=require('./helpers.cjs'),{arrange}=require('./search-template.cjs');
const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/entry-template-qa');fs.mkdirSync(out,{recursive:true});
const forbidden='Неизвестная или запрещённая операция. Изменения базы требуют подписанного сохранения.';
async function proxy(backend){
 const counts={wrongEntry:0,templateRpc:0};
 const server=http.createServer(async(req,res)=>{
  const chunks=[];for await(const c of req)chunks.push(c);const body=Buffer.concat(chunks);
  if(req.url.startsWith('/api/invoke')){const request=JSON.parse(body.toString());if(request.command==='production_excel_template'){counts.templateRpc++;res.writeHead(400,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store'});res.end(JSON.stringify({ok:false,error:forbidden}));return;}}
  const root=req.url==='/'||req.url.startsWith('/?')||req.url==='/index.html';
  if(root)counts.wrongEntry++;
  const headers={...req.headers,host:new URL(backend.url).host,...(req.headers.origin?{origin:backend.url}:{}),...(root?{'user-agent':'Mozilla/5.0 AppleWebKit/537.36 Chrome/131.0.0.0 Edg/131.0.0.0'}:{})};
  const upstream=http.request(backend.url+req.url,{method:req.method,headers},r=>{res.writeHead(r.statusCode,r.headers);r.pipe(res);});upstream.on('error',e=>{res.writeHead(502);res.end(String(e));});upstream.end(body);
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));return {url:'http://127.0.0.1:'+server.address().port,counts,stop:()=>new Promise(resolve=>server.close(resolve))};
}
async function harness(url,source,name,probe=false){
 const script=path.join(out,name+'-driver.js'),report=path.join(out,name+'.json');
 const code=require('@babel/core').transformSync(source,{sourceType:'script',presets:[[require('@babel/preset-env'),{targets:{ie:'11'},modules:false,useBuiltIns:false}]]}).code;
 require('acorn').parse(code,{ecmaVersion:5});fs.writeFileSync(script,code);
 const child=spawn(process.env.IE_HARNESS,[url,script,report],{stdio:'inherit',env:{...process.env,IE_ENTRY_PROBE:probe?'1':'0'}});
 const exit=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);setTimeout(()=>{child.kill();reject(Error('Entry harness timeout'))},200000).unref()});
 const result=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(exit,0,JSON.stringify(result));assert.equal(result.error,null,JSON.stringify(result));assert.equal(result.documentMode,11);return result;
}
async function baseline(){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'production-entry-baseline-'));fs.cpSync(process.env.BASELINE_FRONTEND,path.join(root,'frontend'),{recursive:true});
 const backend=await startServer({root,tls:false}),config=await arrange(backend),server=await proxy(backend);
 try{
  const entry=await harness(server.url,"var result={error:null,documentMode:document.documentMode};try{if(window.Production&&Production.backend)throw Error('Baseline unexpectedly loaded');if(document.documentElement.className.indexOf('legacy-ie')>=0)throw Error('Baseline unexpectedly selected compatibility layout');var fields=document.querySelectorAll('.project-grid>.field');result.fieldWidths=[];for(var i=0;i<fields.length;i++)result.fieldWidths.push(fields[i].offsetWidth);result.menuVisible=getComputedStyle(document.querySelector('.more-actions-menu')).display!=='none';if(!result.menuVisible)throw Error('Reported open menu not reproduced');result.reproduced=true;window.external.Capture('broken-modern-entry');}catch(e){result.error=String(e)}window.external.Report(JSON.stringify(result));",'baseline-entry',true);
  const source='window.__offlineUser='+JSON.stringify(config.users[0])+';'+`(async function(){var result={error:null,documentMode:document.documentMode};function el(id){return document.getElementById(id)}function pause(){return new Promise(function(r){setTimeout(r,60)})}async function wait(f){var end=Date.now()+15000;while(Date.now()<end){if(f())return;await pause()}throw Error('Baseline download timeout')}try{await wait(function(){return el('networkLoginUser').options.length===3});el('networkLoginUser').value=String(window.__offlineUser.id);el('networkLoginPin').value='739201';el('networkLoginBtn').click();await wait(function(){return !el('networkLoginModal').classList.contains('open')});await wait(function(){return el('onboardingModal').classList.contains('open')});document.querySelector('[data-close="onboardingModal"]').click();document.querySelector('.more-actions>summary').click();el('downloadImportTemplateBtn').click();await wait(function(){return !el('downloadImportTemplateBtn').disabled});result.message=el('messageArea').textContent;if(result.message.indexOf('Неизвестная или запрещённая операция')<0)throw Error('Wrong baseline error: '+result.message);result.reproduced=true;window.external.Capture('broken-template');}catch(e){result.error=String(e)}window.external.Report(JSON.stringify(result))}());`;
  const template=await harness(server.url+'/legacy/index.html',source,'baseline-template');assert.equal(server.counts.templateRpc,1);console.log('REPRODUCED real IE: wrong-entry collapsed layout and unknown-operation template error');return {entry,template};
 }finally{await server.stop();await backend.stop()}
}
async function native(){
 await baseline();
 const backend=await startServer({tls:false}),config=await arrange(backend),server=await proxy(backend);
 try{
  const source='window.__focusedConfig='+JSON.stringify(config)+';\n'+fs.readFileSync(path.join(repo,'tests/server/ie-entry-layout.js'),'utf8')+'\n'+fs.readFileSync(path.join(repo,'tests/server/search-template-native.js'),'utf8');
  const r=await harness(server.url,source,'ie-corrected-entry');assert.equal(r.roles.length,3);assert.equal(r.downloads.length,3);assert.equal(r.layout.length,3);assert.equal(r.entryPath,'/legacy/index.html');assert.ok(server.counts.wrongEntry>=1);assert.equal(server.counts.templateRpc,0);r.faults=server.counts;fs.writeFileSync(path.join(out,'ie-corrected-entry.json'),JSON.stringify(r,null,2));console.log('PASS real IE: modern-entry recovery, 3/2/1-column layout, menu toggle, search and exact template in all roles without the template RPC');
 }finally{await server.stop();await backend.stop()}
}
async function edge(){
 const {chromium}=require('playwright'),browser=await chromium.launch({headless:true,executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH});
 try{for(const legacy of [false,true]){
  const backend=await startServer({tls:false}),config=await arrange(backend),server=await proxy(backend),context=await browser.newContext({acceptDownloads:true,viewport:{width:1440,height:1000}}),page=await context.newPage(),prefix=legacy?'es5':'edge',result={error:null,roles:[],external:[],faults:null};
  try{
   await context.route('**/*',r=>{const u=new URL(r.request().url());if(u.origin===server.url||u.protocol==='blob:')return r.continue();result.external.push(u.href);return r.abort('blockedbyclient')});const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(server.url+(legacy?'/legacy/index.html':'/'));
   for(let i=0;i<config.users.length;i++){
    const user=config.users[i];await page.setViewportSize({width:[1440,1024,800][i],height:1000});await page.locator('#networkLoginUser').selectOption(String(user.id));await page.locator('#networkLoginPin').fill('739201');await page.locator('#networkLoginBtn').click();await page.waitForSelector('#onboardingModal.open');await page.locator('[data-close="onboardingModal"]').click();
    const before=await page.evaluate(()=>Production.backend.listProjects());await page.locator('.more-actions>summary').click();const pending=page.waitForEvent('download');await page.locator('#downloadImportTemplateBtn').click();const download=await pending,destination=path.join(out,prefix+'-'+user.role+'.xlsx');await download.saveAs(destination);const bytes=fs.readFileSync(destination);assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),config.template.sha256);assert.deepEqual(bytes,Buffer.from(config.template.base64,'base64'));assert.deepEqual(await page.evaluate(()=>Production.backend.listProjects()),before);await page.screenshot({path:path.join(out,prefix+'-'+user.role+'.png'),fullPage:true});
    result.roles.push({role:user.role,actualBrowserDownload:true,bytes:bytes.length,sha256:config.template.sha256,projectsUnchanged:true});await page.locator('#networkLogoutBtn').click();await page.waitForSelector('#networkLoginModal.open');
   }
   assert.deepEqual(errors,[]);assert.deepEqual(result.external,[]);assert.equal(server.counts.templateRpc,0);result.faults=server.counts;console.log('PASS '+prefix+': actual XLSX download in all roles with unavailable legacy template RPC, no external requests');
  }catch(e){result.error=String(e.stack||e);await page.screenshot({path:path.join(out,prefix+'-failure.png'),fullPage:true});throw e;}finally{fs.writeFileSync(path.join(out,prefix+'-offline-template.json'),JSON.stringify(result,null,2));await context.close();await server.stop();await backend.stop();}
 }}finally{await browser.close()}
}
(process.argv.includes('--ie')?native():edge()).catch(e=>{console.error(e);process.exitCode=1});
