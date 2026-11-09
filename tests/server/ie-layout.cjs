const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {startServer,repo}=require('./helpers.cjs');
(async()=>{
 if(process.platform!=='win32')throw Error('A real Windows MSHTML engine is required');
 const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/ie-layout-qa');fs.mkdirSync(out,{recursive:true});
 for(const baseline of [true,false]){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'production-ie-layout-'));
  fs.cpSync(baseline?process.env.BASELINE_FRONTEND:path.join(repo,'frontend'),path.join(root,'frontend'),{recursive:true});
  const server=await startServer({root,tls:false});
  try{
   const resultPath=path.join(out,baseline?'baseline.json':'layout.json');
   const child=spawn(process.env.IE_HARNESS,[server.url,path.join(repo,'dist/ie-layout-test.js'),resultPath],{stdio:'inherit'});
   const code=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);setTimeout(()=>{child.kill();reject(Error('MSHTML layout harness timeout'))},200000).unref()});
   const result=JSON.parse(fs.readFileSync(resultPath,'utf8'));assert.equal(result.documentMode,11);
   if(baseline){assert.notEqual(result.error,null,'The regression must fail on the previously delivered frontend');assert.match(result.error,/Role cards must share a row/);console.log('REPRODUCED: old IE role cards collapse into separate rows');}
   else{assert.equal(code,0,JSON.stringify(result));assert.equal(result.error,null,JSON.stringify(result));assert.equal(result.checks.length,6);console.log('PASS: real MSHTML bootstrap, profile form, responsive role cards and stage dialogs at 1440, 1024 and 800 pixels');}
  }finally{await server.stop()}
 }
})().catch(e=>{console.error(e);process.exitCode=1});
