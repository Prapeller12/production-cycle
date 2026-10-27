const assert=require('node:assert/strict');
const path=require('node:path');
const {pathToFileURL}=require('node:url');

module.exports=async function checkStageCopyAndSigner(browser){
 const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[];
 page.on('pageerror',error=>errors.push(error.message));
 page.on('dialog',dialog=>dialog.accept());
 try{
  await page.addInitScript(()=>{
   localStorage.setItem('production-cycle-onboarding-v1','done');
   const stage={executor:'Сборочный участок',addressees:'Служба качества',start:'2026-10-01',deadline:'2026-11-30',comment:'Комментарий образца'};
   const snapshot={databaseId:23,baseRevision:'original-revision',project:{name:'Проект копирования',orderNo:'924',initiator:'Дирекция',executor:'Цех',enterprise:'Предприятие',start:'2026-10-01',deadline:'2026-12-31'},stages:[
    {...stage,uid:'assembly',seq:5,sort:1,parentUid:null,title:'Сборка',status:'done',completionConfirmation:{documentType:'Акт приёмки',documentReference:'Акт 1',comment:'Выполнено',actorName:'Сотрудник'}},
    {...stage,uid:'inspection',seq:7,sort:1,parentUid:'assembly',title:'Проверка',status:'work'},
    {...stage,uid:'acceptance',seq:10,sort:2,parentUid:null,title:'Приёмка',status:'new'}
   ],nextSeq:11};
   window.__copyMock={active:false,calls:[]};
   window.__TAURI__={core:{invoke:async(command,args)=>{
    const mock=window.__copyMock;mock.calls.push({command,args:args?structuredClone(args):args});
    if(command==='frontend_ready')return null;
    if(command==='network_status')return{enabled:true,active:mock.active,occupied:mock.active?1:0,maximum:3,evictionRequested:false};
    // Identical names ensure the session is matched by ID, not display name or list order.
    if(command==='audit_list_users')return[
     {id:8,displayName:'Сотрудник',role:'project_manager',active:true,keyFingerprint:'FIRST'},
     {id:9,displayName:'Сотрудник',role:'reviewer',active:true,keyFingerprint:'SECOND'},
     {id:7,displayName:'Администратор',role:'admin',active:true,keyFingerprint:'ADMIN'}
    ];
    if(command==='network_login'){mock.active=true;return{state:'active',occupied:1,maximum:3}}
    if(command==='network_logout'){mock.active=false;return null}
    if(command==='network_save_draft')return{location:'shared',savedAt:'2026-10-27'};
    if(command==='production_list_projects')return[{projectId:23,orderNo:'924',name:'Проект копирования',enterprise:'Предприятие',deadline:'2026-12-31',stageCount:3,doneCount:1,donePercent:33}];
    if(command==='production_load_snapshot_by_id')return structuredClone(snapshot);
    if(command==='production_validate')return{valid:true,errors:[],warnings:[]};
    if(command==='audit_save_snapshot')return{projectId:23,baseRevision:'saved-revision',backupWarning:null};
    if(command==='production_list_dictionary')return[];
    throw Error('Unexpected command '+command);
   }}};
  });
  const login=async id=>{
   await page.waitForSelector('#networkLoginModal.open');await page.selectOption('#networkLoginUser',String(id));
   await page.fill('#networkLoginPin','123456');await page.click('#networkLoginBtn');
   await page.waitForFunction(()=>!document.querySelector('#networkLoginModal').classList.contains('open'));
  };
  const load=async()=>{await page.click('#loadJsonBtn');await page.click('[data-open-project-id="23"]')};
  const cancel=async id=>page.locator(`#${id} [data-close="${id}"]`).first().click();
  const openSave=async expected=>{await page.click('#saveJsonBtn');await page.waitForSelector('#saveConfirmationModal.open');assert.equal(await page.inputValue('#saveSigner'),String(expected));assert.equal(await page.inputValue('#saveSignerPin'),'')};
  await page.goto(pathToFileURL(path.resolve(__dirname,'../frontend/index.html')).href);
  await login(9);await load();await openSave(9);
  await page.selectOption('#saveSigner','8');await page.fill('#saveSignerPin','654321');await cancel('saveConfirmationModal');
  await openSave(9);await cancel('saveConfirmationModal');
  await page.selectOption('#completionFilter','all');
  const original=page.locator('tr[data-stage-uid="assembly"]'),nested=page.locator('tr[data-stage-uid="inspection"]');
  await original.locator('[data-act="copy"]').click();
  assert.equal(await page.locator('#stageModalTitle').innerText(),'Копирование этапа');
  for(const [id,value] of Object.entries({stageTitle:'Сборка — копия',stageParent:'',stageExecutor:'Сборочный участок',stageAddressees:'Служба качества',stageStart:'2026-10-01',stageDeadline:'2026-11-30',stageComment:'Комментарий образца',stageStatus:'new'}))assert.equal(await page.inputValue('#'+id),value);
  assert.equal(await page.locator('#stageIdPreview').innerText(),'924-011');
  assert.equal(await page.locator('#createChildStageBtn').isVisible(),false);
  await cancel('stageModal');assert.equal(await page.locator('#treeBody tr[data-stage-uid]').count(),3);
  await original.locator('[data-act="copy"]').click();assert.equal(await page.locator('#stageIdPreview').innerText(),'924-011');
  await page.fill('#stageTitle','Новая сборка');await page.click('#saveStageBtn');
  assert.equal(await page.locator('#treeBody tr[data-stage-uid]').count(),4);
  assert.equal(await page.locator('#saveConfirmationModal.open').count(),0);
  await nested.locator('[data-act="copy"]').click();assert.equal(await page.inputValue('#stageParent'),'assembly');
  assert.equal(await page.locator('#stageIdPreview').innerText(),'924-012');
  await page.fill('#stageTitle','Новая проверка');await page.click('#saveStageBtn');
  await openSave(9);await page.fill('#saveSignerPin','123456');await page.click('#confirmSignedSaveBtn');
  await page.waitForFunction(()=>!document.querySelector('#saveConfirmationModal').classList.contains('open'));
  const saved=await page.evaluate(()=>window.__copyMock.calls.findLast(x=>x.command==='audit_save_snapshot').args);
  assert.equal(saved.userId,9);assert.equal(saved.snapshot.databaseId,23);assert.equal(saved.snapshot.baseRevision,'original-revision');
  assert.equal(saved.snapshot.stages.length,5);assert.equal(saved.snapshot.nextSeq,13);assert.equal(saved.evidence,null);
  assert.equal(new Set(saved.snapshot.stages.map(x=>x.uid)).size,5);assert.equal(new Set(saved.snapshot.stages.map(x=>x.seq)).size,5);
  const assembly=saved.snapshot.stages.find(x=>x.uid==='assembly'),copy=saved.snapshot.stages.find(x=>x.seq===11),childCopy=saved.snapshot.stages.find(x=>x.seq===12);
  assert.equal(assembly.title,'Сборка');assert.equal(assembly.status,'done');assert.equal(assembly.completionConfirmation.documentReference,'Акт 1');
  assert.equal(copy.title,'Новая сборка');assert.equal(copy.status,'new');assert.equal(copy.parentUid,null);assert.equal(copy.completionConfirmation,undefined);
  assert.equal(childCopy.title,'Новая проверка');assert.equal(childCopy.parentUid,'assembly');assert.equal(childCopy.status,'new');assert.equal(childCopy.completionConfirmation,undefined);
  assert.notEqual(copy.uid,assembly.uid);assert.equal(saved.snapshot.stages.some(x=>x.parentUid===copy.uid),false);
  // Administrator-only reopening must retain its existing role restriction.
  await original.locator('[data-act="edit"]').click();await page.selectOption('#stageStatus','work');await page.click('#saveStageBtn');
  await page.waitForSelector('#saveConfirmationModal.open');assert.deepEqual(await page.locator('#saveSigner option').evaluateAll(options=>options.map(x=>x.value)),['7']);
  await cancel('saveConfirmationModal');
  // Explicit logout and involuntary session loss must both discard the previous identity.
  await page.click('#networkLogoutBtn');await login(8);await load();await openSave(8);await cancel('saveConfirmationModal');
  await page.evaluate(()=>window.__copyMock.active=false);await login(9);await openSave(9);await cancel('saveConfirmationModal');
  await page.setViewportSize({width:760,height:900});
  const button=page.locator('tr[data-stage-uid="inspection"] [data-act="copy"]');await button.scrollIntoViewIfNeeded();
  const bounds=await button.boundingBox();assert.ok(bounds.width>60&&bounds.x>=0&&bounds.x+bounds.width<=760);
  await button.click();assert.equal(await page.inputValue('#stageTitle'),'Проверка — копия');await cancel('stageModal');
  assert.deepEqual(errors,[]);
  console.log('Browser: session signer, role restrictions, identity changes and independent stage copies passed.');
 }catch(error){
  console.error('Stage copy and signer state:',await page.evaluate(()=>({login:document.querySelector('#networkLoginModal')?.className,status:document.querySelector('#networkLoginStatus')?.textContent,commands:window.__copyMock?.calls.map(x=>x.command)})));
  console.error('Page errors:',errors);throw error;
 }finally{await page.close()}
};
