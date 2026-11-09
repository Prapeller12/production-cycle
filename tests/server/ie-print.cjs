const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {startServer,repo}=require('./helpers.cjs');
(async()=>{
 if(process.platform!=='win32')throw Error('Real MSHTML and Windows print dialogs are required');
 const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/button-qa');fs.mkdirSync(out,{recursive:true});
 for(const baseline of [true,false]){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'production-print-'));
  fs.cpSync(baseline?process.env.BASELINE_FRONTEND:path.join(repo,'frontend'),path.join(root,'frontend'),{recursive:true});
  const server=await startServer({root,tls:false});
  try{
   const report=path.join(out,baseline?'ie-print-baseline.json':'ie-native-print.json');
   const child=spawn(process.env.IE_HARNESS,[server.url+(baseline?'/?baseline-print=1':''),path.join(repo,'dist/ie-print-test.js'),report],{stdio:'inherit',env:{...process.env,IE_PRINT_DIALOGS:baseline?'0':'1'}});
   const code=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);setTimeout(()=>{child.kill();reject(Error('IE print harness timeout'))},200000).unref()});
   const result=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(result.documentMode,11);
   if(baseline){assert.notEqual(result.error,null,'The previous release must reproduce the Gantt failure');assert.equal(result.calls,0);assert.ok(result.events.length>=2);console.log('REPRODUCED IE print failure:',JSON.stringify(result));}
   else{assert.equal(code,0,JSON.stringify(result));assert.equal(result.error,null,JSON.stringify(result));assert.equal(result.nativeDialogs,10);console.log('PASS: actual MSHTML opened and cancelled 10 native Print dialogs across Gantt scales, A3/A4, structure and report');}
  }finally{await server.stop()}
 }
})().catch(error=>{console.error(error);process.exitCode=1});
