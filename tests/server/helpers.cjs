const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net'),http=require('node:http'),https=require('node:https'),{spawn}=require('node:child_process');
const repo=path.resolve(__dirname,'../..');
async function freePort(){return new Promise((resolve,reject)=>{const s=net.createServer();s.on('error',reject);s.listen(0,'127.0.0.1',()=>{const port=s.address().port;s.close(()=>resolve(port))})})}
async function startServer(options={}){
 const port=await freePort(),root=options.root||fs.mkdtempSync(path.join(os.tmpdir(),'production-cycle-server-'));
 fs.mkdirSync(path.join(root,'config'),{recursive:true});
 if(!fs.existsSync(path.join(root,'frontend')))fs.cpSync(path.join(repo,'frontend'),path.join(root,'frontend'),{recursive:true});
 const config={...JSON.parse(fs.readFileSync(path.join(repo,'config/server.json'))),listen:'127.0.0.1:'+port,publicUrl:(options.tls===false?'http':'https')+'://127.0.0.1:'+port,tls:options.tls!==false};
 fs.writeFileSync(path.join(root,'config/server.json'),JSON.stringify(config,null,2));
 const binary=process.env.SERVER_EXE||path.join(repo,'server/target/debug/production-cycle-server'+(process.platform==='win32'?'.exe':''));
 const child=spawn(binary,['--serve','--root',root],{stdio:['ignore','pipe','pipe'],windowsHide:true});let log='',exited=false;
 child.stdout.on('data',b=>log=(log+b).slice(-50000));child.stderr.on('data',b=>log=(log+b).slice(-50000));child.on('exit',()=>exited=true);
 child.on('error',e=>log+=String(e));
 const url=config.publicUrl;
 function request(resource,method='GET',body=null,headers={}){return new Promise((resolve,reject)=>{
  const bytes=body===null?null:Buffer.from(typeof body==='string'?body:JSON.stringify(body));
  const ca=config.tls&&fs.existsSync(path.join(root,'tls/server.pem'))?fs.readFileSync(path.join(root,'tls/server.pem')):undefined;
  const req=(config.tls?https:http).request(url+resource,{method,ca,agent:false,...(headers.Host?{servername:'localhost'}:{}),headers:{...(bytes?{'Content-Length':bytes.length}:{}),...headers}},response=>{const chunks=[];response.on('data',b=>chunks.push(b));response.on('end',()=>{const text=Buffer.concat(chunks).toString('utf8');let json;try{json=JSON.parse(text)}catch{}resolve({status:response.statusCode,headers:response.headers,text,json})})});req.on('error',reject);req.setTimeout(30000,()=>req.destroy(Error('Request timeout')));if(bytes)req.write(bytes);req.end();
 })}
 const deadline=Date.now()+30000;let ready=false;
 while(Date.now()<deadline&&!exited){try{const r=await request('/health');if(r.status===200){ready=true;break}}catch{}await new Promise(r=>setTimeout(r,100))}
 if(!ready){child.kill();throw Error('Server did not start: '+log)}
 function client(){let cookie='';return{request,async invoke(command,args={}){const r=await request('/api/invoke','POST',{command,args},{'Content-Type':'application/json','X-Production-Request':'1','Origin':url,...(cookie?{Cookie:cookie}:{})});if(r.headers['set-cookie'])cookie=r.headers['set-cookie'][0].split(';')[0];if(!r.json?.ok)throw Object.assign(Error(r.json?.error||r.text),{status:r.status});return r.json.result;},get cookie(){return cookie}}}
 async function stop(){if(!exited){child.kill();await new Promise(r=>{child.once('exit',r);setTimeout(r,5000).unref()})}}
 return{root,url,config,child,request,client,stop,log:()=>log,binary};
}
module.exports={startServer,repo};
