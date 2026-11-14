const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process'),{startServer,repo}=require('./helpers.cjs');
(async()=>{
 if(process.platform!=='win32')throw Error('The real MSHTML test requires Windows');
 const server=await startServer({tls:false}),out=process.env.SERVER_QA_DIR||path.join(repo,'dist/server-qa');fs.mkdirSync(out,{recursive:true});
 try{
  const resultPath=path.join(out,'ie11-mshtml.json');
  const child=spawn(process.env.IE_HARNESS,[server.url,path.join(repo,'dist/ie-test.js'),resultPath],{stdio:'inherit'});
  const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);setTimeout(()=>{child.kill();reject(Error('MSHTML harness timeout'))},200000).unref()});
  assert.ok(fs.existsSync(resultPath),'No MSHTML report');const result=JSON.parse(fs.readFileSync(resultPath,'utf8'));
  assert.equal(code,0,JSON.stringify(result));assert.equal(result.error,null,JSON.stringify(result));assert.equal(result.documentMode,11,'Not the IE11 engine');assert.ok(result.checks.length>=8);
  console.log('MSHTML documentMode=11: real server, bootstrap, full UI operations and logout passed');
 }finally{await server.stop()}
})().catch(e=>{console.error(e);process.exitCode=1});
