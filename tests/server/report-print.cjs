const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict'),{spawn,spawnSync}=require('node:child_process'),{startServer,repo}=require('./helpers.cjs'),fixture=require('./report-fixture.cjs');
const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/report-print-qa');fs.mkdirSync(out,{recursive:true});
async function user(server){const c=server.client();await c.invoke('network_login',{userId:null,pin:''});const u=await c.invoke('audit_create_user',{input:{displayName:'Администратор проверки отчёта',role:'admin',pin:'739201'}});await c.invoke('network_logout');return u.id;}
function driver(id,count){return 'window.__reportUserId='+id+';window.__reportFixture='+JSON.stringify(fixture(count))+';\n'+fs.readFileSync(path.join(repo,'tests/server/report-print-ui.js'),'utf8');}
function printer(pdf,orientation,remove=false){const p=spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(repo,'tests/server/prepare-pdf-printer.ps1'),'-OutputPath',pdf,'-Orientation',orientation,...(remove?['-Remove']:[])],{encoding:'utf8',timeout:120000});if(p.status!==0)throw Error('QA PDF printer failed: '+p.stdout+'\n'+p.stderr);console.log(p.stdout.trim());}
async function native(){
 for(const test of [{name:'native-before-portrait',baseline:true,orientation:'portrait',count:9},{name:'native-after-portrait',orientation:'portrait',count:9},{name:'native-after-landscape',orientation:'landscape',count:9},{name:'native-after-many-portrait',orientation:'portrait',count:45}]){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'production-native-report-'));fs.cpSync(test.baseline?process.env.BASELINE_FRONTEND:path.join(repo,'frontend'),path.join(root,'frontend'),{recursive:true});
  const server=await startServer({root,tls:false}),id=await user(server),pdf=path.join(out,test.name+'.pdf');
  try{
   printer(pdf,test.orientation);const source=require('@babel/core').transformSync(driver(id,test.count),{sourceType:'script',presets:[[require('@babel/preset-env'),{targets:{ie:'11'},modules:false,useBuiltIns:false}]]}).code;require('acorn').parse(source,{ecmaVersion:5});const script=path.join(out,test.name+'-driver.js'),report=path.join(out,test.name+'.json');fs.writeFileSync(script,source);
   const child=spawn(process.env.IE_HARNESS,[server.url,script,report],{stdio:'inherit',env:{...process.env,IE_PDF_OUTPUT:pdf}}),exit=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);setTimeout(()=>{child.kill();reject(Error('Native report timeout'))},200000).unref()});
   const r=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(exit,0,JSON.stringify(r));assert.equal(r.error,null,JSON.stringify(r));assert.equal(r.documentMode,11);assert.equal(r.actualNativePdf,true);assert.equal(r.rows,test.count);assert.ok(fs.statSync(pdf).size>1000);console.log('PRINTED actual MSHTML '+test.name+' with '+r.rows+' stages');
  }finally{try{printer(pdf,test.orientation,true)}finally{await server.stop()}}
 }
}
async function edge(){
 const {chromium}=require('playwright'),browser=await chromium.launch({headless:true,executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH});
 try{for(const legacy of [false,true])for(const orientation of ['portrait','landscape']){
  const server=await startServer(),id=await user(server),context=await browser.newContext({ignoreHTTPSErrors:true,viewport:{width:1440,height:1000}}),page=await context.newPage(),name=(legacy?'es5':'edge')+'-report-'+orientation,errors=[];
  try{
   page.on('pageerror',e=>errors.push(e.message));await context.route('**/*',r=>new URL(r.request().url()).origin===server.url?r.continue():r.abort('blockedbyclient'));await page.goto(server.url+(legacy?'/?legacy=1':'/'));await page.waitForFunction(()=>window.ProductionServer&&window.Production?.backend);const r=await page.evaluate(driver(id,9));assert.equal(r.error,null,JSON.stringify(r));assert.equal(r.rows,9);assert.deepEqual(errors,[]);await page.addStyleTag({content:'@media print{@page{size:A4 '+orientation+';margin:8mm}}'});await page.pdf({path:path.join(out,name+'.pdf'),preferCSSPageSize:true,printBackground:true});fs.writeFileSync(path.join(out,name+'.json'),JSON.stringify(r,null,2));console.log('PRINTED '+name);
  }finally{await context.close();await server.stop()}
 }}finally{await browser.close()}
}
(process.argv.includes('--ie')?native():edge()).catch(e=>{console.error(e);process.exitCode=1});
