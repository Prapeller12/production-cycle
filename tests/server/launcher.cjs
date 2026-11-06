const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net'),http=require('node:http'),https=require('node:https'),assert=require('node:assert/strict'),{spawn}=require('node:child_process'),{repo}=require('./helpers.cjs');
const binary=process.env.SERVER_EXE||path.join(repo,'server/target/debug/production-cycle-server'+(process.platform==='win32'?'.exe':''));
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function freePort(){return new Promise((resolve,reject)=>{const s=net.createServer();s.on('error',reject);s.listen(0,'127.0.0.1',()=>{const port=s.address().port;s.close(()=>resolve(port))})})}
function child(args){const process=spawn(binary,args,{stdio:['ignore','pipe','pipe'],windowsHide:true});let output='';process.stdout.on('data',b=>output+=b);process.stderr.on('data',b=>output+=b);const done=new Promise((resolve,reject)=>{process.on('error',reject);process.on('exit',(code,signal)=>resolve({code,signal,output}))});return{process,done,output:()=>output}}
async function run(args){const c=child(args);const timer=setTimeout(()=>c.process.kill(),15000);try{return await c.done}finally{clearTimeout(timer)}}
function request(url,root,resource,method='GET',headers={}){return new Promise((resolve,reject)=>{const secure=url.startsWith('https:'),ca=secure?fs.readFileSync(path.join(root,'tls/server.pem')):undefined;const req=(secure?https:http).request(url+resource,{method,ca,agent:false,headers},response=>{let text='';response.on('data',b=>text+=b);response.on('end',()=>{let json;try{json=JSON.parse(text)}catch{}resolve({status:response.statusCode,text,json})})});req.on('error',reject);req.setTimeout(2000,()=>req.destroy(Error('timeout')));req.end()})}
async function ready(url,root){const deadline=Date.now()+20000;while(Date.now()<deadline){try{const r=await request(url,root,'/health');if(r.status===200)return r.json}catch{}await pause(100)}throw Error('Launcher did not become ready')}
(async()=>{
 const checks=[];
 for(const tls of [false,true]){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'Производственный цикл запуск ')),port=await freePort();
  fs.mkdirSync(path.join(root,'config'));fs.cpSync(path.join(repo,'frontend'),path.join(root,'frontend'),{recursive:true});
  const config={...JSON.parse(fs.readFileSync(path.join(repo,'config/server.json'))),listen:'127.0.0.1:'+port,publicUrl:(tls?'https':'http')+'://127.0.0.1:'+port,tls};
  fs.writeFileSync(path.join(root,'config/server.json'),JSON.stringify(config));
  const url=config.publicUrl;
  const first=child(['--root',root]);
  try{
   let status;try{status=await ready(url,root)}catch(error){throw Error(error.message+'; exit='+first.process.exitCode+'; '+first.output())}assert.equal(status.application,'production-cycle');assert.equal(status.mode,'manual');
   const runtimeFile=path.join(root,'server-data/running.json'),runtime=JSON.parse(fs.readFileSync(runtimeFile));assert.equal(runtime.pid,first.process.pid);
   const checked=await run(['--check','--root',root]);assert.equal(checked.code,0,checked.output);
   assert.equal((await request(url,root,'/__local/stop','POST')).status,403);
   assert.equal((await request(url,root,'/__local/stop','POST',{'X-Production-Control':'wrong'})).status,403);
   assert.equal((await request(url,root,'/__local/stop','POST',{'X-Production-Control':runtime.token,Origin:'https://evil.invalid'})).status,403);
   if(process.platform==='win32'){
    const second=await run(['--root',root]);assert.equal(second.code,0,second.output);assert.equal(JSON.parse(fs.readFileSync(runtimeFile)).pid,first.process.pid,'Repeated clicks must reuse the running server');
    const powershell=spawn('powershell.exe',['-NoProfile','-Command',"Get-CimInstance Win32_Process -Filter \"name='msedge.exe'\" | Select-Object -ExpandProperty CommandLine"],{stdio:['ignore','pipe','pipe']});
    let commands='';for await(const chunk of powershell.stdout)commands+=chunk;
    assert.ok(commands.includes('--new-window')&&commands.includes(':'+port),'Installed Edge was not launched with the program address');
   }
   const db=path.join(root,'server-data/production-cycle.db'),before=fs.statSync(db).size;
   const stopped=await run(['--stop','--root',root]);assert.equal(stopped.code,0,stopped.output);
   const finished=await first.done;assert.equal(finished.code,0,finished.output);assert.ok(!fs.existsSync(runtimeFile),'Transient control file must be removed after graceful shutdown');assert.equal(fs.statSync(db).size,before);
   checks.push((tls?'HTTPS':'HTTP')+': readiness, native health client, authenticated local shutdown, preserved database'+(process.platform==='win32'?', real Edge launch and duplicate-click reuse':''));
  }finally{first.process.kill()}
 }
 const out=process.env.SERVER_QA_DIR||path.join(repo,'dist/server-qa');fs.mkdirSync(out,{recursive:true});fs.writeFileSync(path.join(out,'launcher.json'),JSON.stringify({checks},null,2));console.log(checks.join('\n'));
})().catch(e=>{console.error(e);process.exitCode=1});
