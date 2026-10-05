// Executed inside the packaged Windows WebView, against real IPC and SQLite.
(async()=>{
 const checks=[],errors=[];let phase='boot',error=null;
 const $=id=>document.getElementById(id),sleep=ms=>new Promise(r=>setTimeout(r,ms));
 const assert=(ok,message)=>{if(!ok)throw Error(message)};
 const wait=async(test,label)=>{const until=Date.now()+30000;while(Date.now()<until){if(await test())return;await sleep(100)}throw Error('Timeout: '+label+'; '+($('auditUserError')?.innerText||'')+'; '+($('saveConfirmationError')?.innerText||'')+'; '+($('networkLoginStatus')?.innerText||''))};
 const visible=e=>e&&e.getClientRects().length>0&&getComputedStyle(e).visibility!=='hidden';
 const fill=(id,value)=>{const e=$(id);assert(visible(e),'Field not visible: '+id);e.value=String(value);e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}))};
 const click=selector=>{const e=selector.startsWith('#')?document.querySelector(selector):document.querySelector(selector);assert(visible(e)&&!e.disabled,'Button unavailable: '+selector);e.click()};
 const open=id=>$(id).classList.contains('open');
 const more=selector=>{const menu=document.querySelector('.more-actions');menu.open=true;click(selector)};
 window.addEventListener('error',e=>errors.push(e.message));window.addEventListener('unhandledrejection',e=>errors.push(String(e.reason)));
 window.confirm=()=>true;
 try{
  await wait(()=>window.Production?.backend,'frontend');const B=Production.backend;
  const users=await B.listAuditUsers();let admin,manager,reviewer;
  if(!users.length){
   phase='first-run administrator form';
   await wait(()=>visible($('newAuditUserName')),'first administrator form (no blocking login)');
   assert(!open('networkLoginModal'),'Login blocks bootstrap');
   fill('newAuditUserName','Администратор проверки');fill('newAuditUserPin','739201');fill('newAuditUserPinConfirm','wrong1');click('#createAuditUserBtn');
   assert($('auditUserError').innerText.includes('не совпадают'),'PIN mismatch not explained');
   fill('newAuditUserPinConfirm','739201');click('#createAuditUserBtn');
   await wait(async()=> (await B.listAuditUsers()).length===1&&!$('createAuditUserBtn').disabled,'create first admin through UI');
   admin=(await B.listAuditUsers())[0];assert(admin.role==='admin','First role is not administrator');
   assert((await B.networkStatus()).displayName===admin.displayName,'Bootstrap session not bound to administrator');
   checks.push('clean ZIP: first administrator created through visible UI, PIN validation, session binding');
   for(const [name,role] of [['Руководитель проверки','project_manager'],['Проверяющий проверки','reviewer']]){
    $('createUserDetails').open=true;fill('newAuditUserName',name);fill('newAuditUserRole',role);fill('newAuditUserPin','739201');fill('newAuditUserPinConfirm','739201');fill('authorizingUser',admin.id);fill('authorizingUserPin','739201');click('#createAuditUserBtn');
    await wait(async()=> (await B.listAuditUsers()).some(u=>u.displayName===name)&&!$('createAuditUserBtn').disabled,'create '+role);
   }
   checks.push('project manager and reviewer creation through UI');click('#closeAdminBtn');
  }else{
   phase='relaunch login';admin=users.find(u=>u.role==='admin');
   await wait(()=>open('networkLoginModal')&&$('networkLoginUser').options.length,'login form');
   fill('networkLoginUser',admin.id);fill('networkLoginPin','wrong1');click('#networkLoginBtn');
   await wait(()=>$('networkLoginStatus').classList.contains('error')&&!$('networkLoginBtn').disabled,'wrong PIN rejection');assert(open('networkLoginModal'),'Wrong PIN admitted user');
   fill('networkLoginPin','739201');click('#networkLoginBtn');await wait(()=>!open('networkLoginModal'),'authenticated login');
   await sleep(400);if(open('onboardingModal'))click('[data-close="onboardingModal"]');
   const projects=await B.listProjects();assert(projects.length>=2,'Projects lost after restart');
   click('#loadJsonBtn');await wait(()=>document.querySelector('[data-open-project-id]'),'saved project selector');click('[data-open-project-id="'+projects[0].projectId+'"]');await wait(()=>!open('projectsModal'),'reopen project');assert($('projectName').value===projects[0].name,'Wrong project reopened');
   const verification=await B.verifyAuditLog(admin.id,'739201');assert(verification.invalidEvents===0,'Audit invalid after restart');
   checks.push('second EXE launch: profiles persisted, wrong PIN denied, correct login, saved project reopened, signatures valid');
  }
  manager=(await B.listAuditUsers()).find(u=>u.role==='project_manager');reviewer=(await B.listAuditUsers()).find(u=>u.role==='reviewer');
  assert(admin&&manager&&reviewer,'Missing roles');
  if(!users.length){
   phase='project and hierarchy UI';
   for(const [id,value] of Object.entries({projectName:'Сетевая проверка проекта',orderNo:'NET-TEST',initiator:'Инициатор',executor:'Исполнитель',projectAddressees:'Предприятие',projectStart:'2026-09-01',projectDeadline:'2026-11-30'}))fill(id,value);
   click('.stage-primary-btn');click('#saveStageBtn');assert($('stageTitle').getAttribute('aria-invalid')==='true','Required title not highlighted');
   fill('stageTitle','Родительский этап');fill('stageExecutor','Исполнитель');fill('stageAddressees','Предприятие');fill('stageStart','2026-09-01');fill('stageDeadline','2026-10-30');fill('stageStatus','work');fill('stageComment','Комментарий родителя');click('#saveStageBtn');
   await wait(()=>!open('stageModal'),'create stage');click('#treeBody [data-act="edit"]');click('#createChildStageBtn');fill('stageTitle','Дочерний этап');fill('stageDeadline','2026-10-15');fill('stageStatus','work');click('#saveStageBtn');await wait(()=>!open('stageModal'),'create child');
   const sign=async(user,comment,evidence=false)=>{
    await wait(()=>open('saveConfirmationModal'),'sign dialog');fill('saveSigner',user.id);fill('saveSignerPin','739201');fill('saveChangeComment',comment);
    if(evidence){fill('evidenceType','Акт приёмки');fill('evidenceReference','NET-ACT-1');fill('evidenceComment','Принят только выбранный этап')}
    click('#confirmSignedSaveBtn');await wait(()=>!open('saveConfirmationModal'),'signed save');
   };
   click('#saveJsonBtn');await sign(manager,'Создание проекта через интерфейс');
   let projects=await B.listProjects(),project=projects.find(p=>p.orderNo==='NET-TEST');assert(project,'Project not saved');let state=await B.loadById(project.projectId);assert(state.stages.length===2,'Hierarchy lost');const parent=state.stages.find(s=>!s.parentUid),child=state.stages.find(s=>s.parentUid);assert(child.parentUid===parent.uid,'Wrong hierarchy');
   checks.push('project fields, required-field errors, parent and child creation, manager signed save, SQLite reload');
   phase='single-stage completion';click('#treeBody tr[data-stage-uid="'+parent.uid+'"] [data-act="edit"]');fill('stageStatus','done');assert($('completionScope').value==='stage','Unsafe default completion scope');click('#saveStageBtn');
   await wait(()=>open('saveConfirmationModal'),'completion dialog');click('#confirmSignedSaveBtn');assert($('saveSignerPin').getAttribute('aria-invalid')==='true','Required signature field not highlighted');await sign(manager,'Завершён один этап',true);
   state=await B.loadById(project.projectId);assert(state.stages.find(s=>s.uid===parent.uid).status==='done'&&state.stages.find(s=>s.uid===child.uid).status==='work','Completion cascaded unexpectedly');assert(!document.querySelector('#treeBody tr[data-stage-uid="'+parent.uid+'"]'),'Completed stage not filtered');fill('completionFilter','all');assert(document.querySelector('#treeBody tr[data-stage-uid="'+parent.uid+'"]'),'Completed stage disappeared permanently');
   const report=await B.report(state);assert(report.donePercent===50,'Wrong completion percent');assert(report.rows.some(r=>r.completionConfirmation?.signatureValid),'Missing verified completion signature');checks.push('single-stage completion, required evidence, child remains open, completion filter, 50% report and signature');
   phase='administrator reopen';click('#treeBody tr[data-stage-uid="'+parent.uid+'"] [data-act="edit"]');fill('stageStatus','work');click('#saveStageBtn');await wait(()=>open('saveConfirmationModal'),'reopen confirmation');assert([...$('saveSigner').options].every(o=>Number(o.value)===admin.id),'Reopening permitted without administrator');await sign(admin,'Возврат на доработку');
   state=await B.loadById(project.projectId);assert(state.stages.every(s=>s.status==='work'),'Reopen not persisted');checks.push('administrator-only return to work');
   phase='duplicate order and exchange';const copy=JSON.parse(JSON.stringify(state));copy.databaseId=null;copy.project.name='Второй проект того же заказа';const saved=await B.saveSigned(copy,{userId:manager.id,pin:'739201',comment:'Другой проект под тем же заказом'});assert(saved.projectId!==project.projectId,'Same order overwrote project');assert((await B.listProjects()).filter(p=>p.orderNo==='NET-TEST').length===2,'Duplicate-order selector data missing');
   const pair=await B.exportPair(state),restored=Production.exchange.importPair('\ufeff'+pair.out,'\ufeff'+pair.in);assert(restored.state.stages.length===2,'TXT import lost stages');const json=Production.projectFile.stringify(state);assert(Production.projectFile.parse(json).stages.length===2,'JSON round trip failed');
   const invoke=window.__TAURI__.core.invoke;await invoke('save_file',{name:'Сетевая проверка.json',text:json});await invoke('save_file',{name:'Сетевая исходящий.txt',text:pair.out});await invoke('save_file',{name:'Сетевая входящий.txt',text:pair.in});checks.push('multiple projects under one order, paired TXT import/export, JSON round trip, native file writes');
   phase='deadline and gantt UI';click('#deadlinesTab');await wait(()=>document.querySelector('#deadlineControlList .deadline-row'),'deadline list');click('#ganttTab');assert($('ganttScale').value==='quarter','Quarter default missing');for(const scale of ['day','week','month','quarter']){fill('ganttScale',scale);assert($('ganttChart').textContent.includes('Родительский этап'),'Gantt missing stage: '+scale)}click('#registryTab');click('#toggleStructureBtn');assert($('toggleStructureBtn').getAttribute('aria-expanded')==='true','Structure toggle failed');checks.push('deadline control, all four Gantt scales, hierarchy expand/collapse');
   phase='administration and backup UI';click('#auditBtn');await wait(()=>open('adminLoginModal'),'admin login');fill('adminLoginPin','739201');click('#confirmAdminLoginBtn');await wait(()=>visible($('auditEvents'))&&document.querySelector('#auditEvents .audit-event'),'signed journal');click('#auditVerificationTab');fill('verifyAdminPin','739201');click('#verifyAuditBtn');await wait(()=>$('auditVerification').innerText.includes('Проверка пройдена'),'signature verification');
   click('#dictionaryTab');await wait(()=>document.querySelector('#dictionaryList .dictionary-row'),'dictionary');assert($('dictionaryList').innerText.includes('Предприятие'),'Saved dictionary absent');
   click('#auditBackupsTab');await wait(()=>document.querySelector('#backupList .backup-row'),'automatic backups');fill('backupAdminPin','739201');click('#createBackupBtn');await wait(()=>$('backupResult').innerText.includes('Копия создана'),'manual backup');
   const backup=(await B.listBackups()).find(b=>b.kind==='manual');assert(backup&&backup.valid,'Manual backup invalid');const restoredBackup=await B.restoreBackup(backup.fileName,admin.id,'739201');assert(restoredBackup.integrityCheck==='ok','Restore integrity failed');assert((await B.listProjects()).length===2,'Restore lost project');checks.push('admin login, signed journal, signature verification, dictionaries, automatic/manual backup and verified restore');click('#closeAdminBtn');
   phase='autosave';fill('projectName','Несохранённый черновик');await sleep(1600);assert((await B.networkStatus()).active,'Session lost during autosave');const draft=await B.networkSaveDraft(state);assert(draft.location==='shared','Shared draft failed');checks.push('active session and shared draft autosave');
  }
  assert(errors.length===0,'Uncaught UI errors: '+errors.join('; '));
 }catch(e){error=phase+': '+String(e)}
 await window.__TAURI__.core.invoke('finish_smoke',{error,checks});
})();
