const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{spawn}=require('node:child_process'),{startServer,repo}=require('./helpers.cjs');
(async()=>{
 if(process.platform!=='win32')throw Error('Real MSHTML is required');
 const server=await startServer({tls:false}),out=process.env.SERVER_QA_DIR||path.join(repo,'dist/button-qa');fs.mkdirSync(out,{recursive:true});
 try{
  const report=path.join(out,'ie-button-audit.json');
  const child=spawn(process.env.IE_HARNESS,[server.url,path.join(repo,'dist/ie-button-test.js'),report],{stdio:'inherit'});
  const code=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);setTimeout(()=>{child.kill();reject(Error('MSHTML button audit timeout'))},200000).unref()});
  const result=JSON.parse(fs.readFileSync(report,'utf8'));assert.equal(code,0,JSON.stringify(result));assert.equal(result.error,null,JSON.stringify(result));assert.equal(result.documentMode,11);assert.deepEqual(result.coverage.missing,[]);assert.ok(result.scenarios.length>=16);
  console.log('PASS real MSHTML button audit:',result.coverage.staticButtons,'static buttons;',result.coverage.dynamicControls,'dynamic controls;',result.scenarios.length,'UI scenarios');
 }finally{await server.stop()}
})().catch(error=>{console.error(error);process.exitCode=1});
