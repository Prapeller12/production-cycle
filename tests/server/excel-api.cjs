const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{startServer,repo}=require('./helpers.cjs');
(async()=>{
 let server=await startServer();const checks=[],out=process.env.SERVER_QA_DIR||path.join(repo,'dist/excel-qa');fs.mkdirSync(out,{recursive:true});
 try{
  const anonymous=server.client();for(const command of ['production_import_excel','production_excel_template','ui_claim_onboarding'])await assert.rejects(()=>anonymous.invoke(command),/Сеанс/);
  const admin=server.client();await admin.invoke('network_login',{userId:null,pin:''});
  const owner=await admin.invoke('audit_create_user',{input:{displayName:'Excel администратор',pin:'739201',role:'admin'}});
  const create=(displayName,role)=>admin.invoke('audit_create_user',{input:{displayName,role,pin:'739201',adminUserId:owner.id,adminPin:'739201'}});
  const manager=await create('Excel руководитель','project_manager'),reviewer=await create('Excel проверяющий','reviewer');
  const m=server.client(),r=server.client();await m.invoke('network_login',{userId:manager.id,pin:'739201'});await r.invoke('network_login',{userId:reviewer.id,pin:'739201'});
  const template=await m.invoke('production_excel_template'),resource=JSON.parse(fs.readFileSync(path.join(repo,'server/resources/project-import-template.json')));
  assert.deepEqual(template,resource);assert.equal(crypto.createHash('sha256').update(Buffer.from(template.base64,'base64')).digest('hex'),template.sha256);
  const parse=(client,base64=template.base64,fileName=template.fileName)=>client.invoke('production_import_excel',{base64,fileName});
  let imported;for(const client of [admin,m,r]){const value=await parse(client);assert.equal(value.snapshot.project.orderNo,'000924-27');assert.equal(value.snapshot.project.start,'2027-01-11');assert.equal(value.snapshot.project.deadline,'2027-03-31');assert.equal(value.rows.length,5);assert.equal(value.snapshot.stages[3].parentUid,'excel-3');assert.ok(value.snapshot.stages.every(s=>s.status==='new'));assert.equal((await client.invoke('production_list_projects')).length,0);imported=value;}
  checks.push('Three roles download exact offline template and preview leading zeros, dates and hierarchy without database writes');
  const fixtures=JSON.parse(fs.readFileSync(path.join(repo,'dist/excel-fixtures.json')));
  for(const name of ['duplicate','missingParent','selfParent','cycle','badDate','emptyTitle','missingSheet','bigDimensions','formula','duplicateHeader'])await assert.rejects(()=>parse(m,fixtures[name]),undefined,name);
  await assert.rejects(()=>parse(m,Buffer.from('broken').toString('base64')));await assert.rejects(()=>parse(m,template.base64,'sample.xls'));await assert.rejects(()=>parse(m,Buffer.alloc(5*1024*1024+1).toString('base64')));
  const epoch=await parse(m,fixtures.epoch1904);assert.deepEqual(epoch.snapshot,imported.snapshot);
  const reversed=await parse(m,fixtures.reordered);assert.equal(reversed.rows[0].code,'Э05');assert.equal(reversed.snapshot.stages.find(s=>s.title===imported.snapshot.stages[3].title).parentUid,'excel-3');
  checks.push('Thirteen invalid-input checks, Excel 1904 calendar and parent-after-child workbook');
  const saved=await m.invoke('audit_save_snapshot',{snapshot:imported.snapshot,userId:manager.id,pin:'739201',comment:'Подписанный импорт Excel',evidence:null});
  assert.equal((await r.invoke('production_load_snapshot_by_id',{projectId:saved.projectId})).stages.length,5);
  for(const client of [m,r]){await client.invoke('audit_authorize_admin',{userId:owner.id,pin:'739201'});await assert.rejects(()=>client.invoke('production_export_txt',{snapshot:imported.snapshot}),/администратор/i);}
  const pair=await admin.invoke('production_export_txt',{snapshot:imported.snapshot});assert.ok(pair.outgoingText,JSON.stringify(pair).slice(0,200));
  checks.push('Signed import persists; registry TXT export restricted by logged-in role even after administrator PIN authorization');
  assert.equal((await m.invoke('ui_claim_onboarding',{userId:reviewer.id})).show,true);assert.equal((await m.invoke('ui_claim_onboarding')).show,false);assert.equal((await r.invoke('ui_claim_onboarding')).show,true);assert.equal((await admin.invoke('ui_claim_onboarding')).show,true);
  const root=server.root;await server.stop();server=await startServer({root});const again=server.client();await again.invoke('network_login',{userId:manager.id,pin:'739201'});
  assert.equal((await again.invoke('ui_claim_onboarding')).show,false);assert.equal((await again.invoke('production_list_projects')).length,1);
  const a=server.client();await a.invoke('network_login',{userId:owner.id,pin:'739201'});assert.equal((await a.invoke('audit_verify_log',{adminUserId:owner.id,adminPin:'739201',projectId:null})).invalidEvents,0);
  checks.push('First-use rules are per authenticated identity, cannot mark another user and survive server restart; signed journal remains valid');
  fs.writeFileSync(path.join(out,'excel-api.json'),JSON.stringify({ok:true,checks},null,2));console.log(JSON.stringify({ok:true,checks},null,2));
 }finally{await server.stop()}
})().catch(error=>{console.error(error);process.exitCode=1});
