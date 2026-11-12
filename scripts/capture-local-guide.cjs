// Documentation only: capture the unchanged released local EXE with disposable training data.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),{spawn,spawnSync}=require('node:child_process'),{chromium}=require('playwright');
const root=process.env.GUIDE_APP_ROOT||'C:/ProductionCycle',out=path.resolve('dist/local-guide-capture');fs.mkdirSync(out,{recursive:true});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const wait=async(f,label)=>{const end=Date.now()+90000;while(Date.now()<end){if(await f())return;await sleep(100)}throw Error('Timeout: '+label)};
const captures=[],pin='739201';
let browser,page,child,admin,manager,reviewer,projectId,base,assembly,procurement;
async function fill(id,value){const e=page.locator('#'+id);if(await e.evaluate(e=>e.tagName==='SELECT'))await e.selectOption(String(value));else await e.fill(String(value));await e.dispatchEvent('change');}
async function click(selector){await page.locator(selector).click();}
async function close(id){if(await page.locator('#'+id).evaluate(e=>e.classList.contains('open')))await click('#'+id+' [data-close="'+id+'"]');}
async function more(id){await page.evaluate(()=>document.querySelector('.more-actions').open=true);await click('#'+id);}
async function shot(name,scope,marks=[]){
 const selectors=Array.isArray(scope)?scope:[scope];await page.locator(selectors[0]).scrollIntoViewIfNeeded();await sleep(160);
 const boxes=await Promise.all(selectors.map(s=>page.locator(s).first().boundingBox()));assert.ok(boxes.every(Boolean),'Missing scope '+name);
 let b={x:Math.min(...boxes.map(x=>x.x)),y:Math.min(...boxes.map(x=>x.y)),right:Math.max(...boxes.map(x=>x.x+x.width)),bottom:Math.max(...boxes.map(x=>x.y+x.height))};b.width=b.right-b.x;b.height=b.bottom-b.y;
 if(selectors.length>1){const scroll=await page.evaluate(()=>({x:scrollX,y:scrollY}));if(b.y<0||b.bottom>1200){await page.evaluate(y=>scrollTo(0,y),Math.max(0,scroll.y+b.y-25));return shot(name,scope,marks);}}
 const points=[];for(const [n,selector,side='left']of marks){const r=await page.locator(selector).first().boundingBox();assert.ok(r,'Missing marker '+name+' '+selector);points.push({n,side,x:r.x-b.x,y:r.y-b.y,width:r.width,height:r.height});}
 const target=path.join(out,name+'.png');
 if(selectors.length===1)await page.locator(selectors[0]).first().screenshot({path:target,scale:'device'});
 else{const scroll=await page.evaluate(()=>({x:scrollX,y:scrollY}));await page.screenshot({path:target,clip:{x:b.x+scroll.x,y:b.y+scroll.y,width:b.width,height:b.height},scale:'device'});}
 captures.push({name,file:name+'.png',width:b.width,height:b.height,marks:points});console.log('Captured',name,Math.round(b.width),Math.round(b.height));fs.writeFileSync(path.join(out,'captures.json'),JSON.stringify(captures,null,2));
}
async function sign(comment,capture=false){await page.waitForSelector('#saveConfirmationModal.open');await fill('saveSigner',manager.id);await fill('saveSignerPin',pin);await fill('saveChangeComment',comment);if(capture)await shot('signature','#saveConfirmationModal .modal',[[1,'#saveSigner'],[2,'#saveSignerPin','right'],[3,'#saveChangeComment'],[4,'#confirmSignedSaveBtn','right']]);await click('#confirmSignedSaveBtn');await page.waitForSelector('#saveConfirmationModal.open',{state:'hidden'});await sleep(160);}
async function all(){await fill('completionFilter','all');if(!await page.locator('#treeBody tr[data-stage-uid]').count())await click('#treeBody [data-act="toggleRoot"]');const t=page.locator('#treeBody [data-act="toggle"][aria-expanded="false"]');while(await t.count())await t.first().click();}
async function stage(title,executor,addressees,start,deadline,status,comment=''){for(const [id,value]of Object.entries({stageTitle:title,stageExecutor:executor,stageAddressees:addressees,stageStart:start,stageDeadline:deadline,stageStatus:status,stageComment:comment}))await fill(id,value);}
async function loadCurrent(){await click('#loadJsonBtn');await page.waitForSelector('[data-open-project-id="'+projectId+'"]');await click('[data-open-project-id="'+projectId+'"]');await page.waitForSelector('#projectsModal.open',{state:'hidden'});await all();}
(async()=>{
 try{
  child=spawn(path.join(root,'production-cycle.exe'),[],{cwd:root,env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9222 --lang=ru-RU'}});child.on('error',console.error);
  await wait(async()=>{try{return(await fetch('http://127.0.0.1:9222/json/version',{signal:AbortSignal.timeout(1500)})).ok}catch{return false}},'native CDP');
  browser=await chromium.connectOverCDP('http://127.0.0.1:9222');const context=browser.contexts()[0];await wait(()=>{page=context.pages().find(p=>p.url().includes('production.localhost'));return page},'native document');
  await page.setViewportSize({width:1200,height:1200});const cdp=await context.newCDPSession(page);await cdp.send('Emulation.setDeviceMetricsOverride',{width:1200,height:1200,deviceScaleFactor:2,mobile:false});
  page.on('dialog',d=>d.accept());await page.evaluate(()=>window.confirm=()=>true);
  await page.waitForFunction(()=>window.ProductionDesktop===true&&window.Production?.backend&&document.getElementById('newAuditUserName').getClientRects().length);
  assert.equal(await page.evaluate(()=>Boolean(window.ProductionServer)),false);
  await fill('newAuditUserName','Учебный администратор');await fill('newAuditUserPin',pin);await fill('newAuditUserPinConfirm',pin);
  await shot('setup','#createUserDetails',[[1,'#newAuditUserName'],[2,'#newAuditUserPin','right'],[3,'#createAuditUserBtn','right']]);
  await click('#createAuditUserBtn');await page.waitForFunction(()=>!document.getElementById('createAuditUserBtn').disabled&&document.getElementById('auditUserError').textContent.includes('Создан профиль'));
  admin=await page.evaluate(async()=>(await Production.backend.listAuditUsers()).find(u=>u.role==='admin'));
  await sleep(500);await close('onboardingModal');
  await page.locator('#createUserDetails').evaluate(e=>e.open=true);await fill('newAuditUserName','Учебный проверяющий');await fill('newAuditUserRole','reviewer');await fill('newAuditUserPin',pin);await fill('newAuditUserPinConfirm',pin);await fill('authorizingUser',admin.id);await fill('authorizingUserPin',pin);
  await shot('users','#createUserDetails',[[1,'#newAuditUserName'],[2,'#newAuditUserRole','right'],[3,'#newAuditUserPin'],[4,'#authorizingUserPin','right'],[5,'#createAuditUserBtn','right']]);
  await click('#createAuditUserBtn');await page.waitForFunction(async()=>!document.getElementById('createAuditUserBtn').disabled&&(await Production.backend.listAuditUsers()).length===2);
  manager=await page.evaluate(async({pin,admin})=>Production.backend.createAuditUser({displayName:'Учебный руководитель',role:'project_manager',pin,adminUserId:admin.id,adminPin:pin}),{pin,admin});
  reviewer=await page.evaluate(async()=>(await Production.backend.listAuditUsers()).find(u=>u.role==='reviewer'));
  await click('#closeAdminBtn');await click('#networkLogoutBtn');await page.waitForSelector('#networkLoginModal.open');await fill('networkLoginUser',manager.id);await fill('networkLoginPin',pin);
  await shot('login','#networkLoginModal .modal',[[1,'#networkLoginUser'],[2,'#networkLoginPin','right'],[3,'#networkLoginBtn','right']]);await click('#networkLoginBtn');await page.waitForSelector('#networkLoginModal.open',{state:'hidden'});await page.waitForSelector('#onboardingModal.open');
  await shot('hints','#onboardingModal .modal',[[1,'#onboardingNextBtn','right'],[2,'#onboardingModal [data-close="onboardingModal"]','right']]);await close('onboardingModal');
  await shot('main-actions',['.appbar','.tabs','.toolbar','#projectSearchForm'],[[1,'#newBtn','top'],[2,'#loadJsonBtn','top'],[3,'#saveJsonBtn','right'],[4,'#projectsSearch'],[5,'.more-actions > summary','right']]);
  for(const [id,value]of Object.entries({projectName:'Изготовление модуля А',orderNo:'924',initiator:'Производственная дирекция',executor:'Сборочный участок',projectStart:'2026-10-01',projectDeadline:'2026-12-31',projectAddressees:'Учебное предприятие'}))await fill(id,value);
  await shot('project','.project-card',[[1,'#projectName'],[2,'#orderNo','right'],[3,'#projectDeadline','right'],[4,'#projectAddressees']]);
  await click('.stage-primary-btn');await stage('Закупка комплектующих','Отдел снабжения','Сборочный участок','2026-10-01','2026-10-15','work','Ожидаем поставку комплектующих');
  await shot('stage-new','#stageModal .modal',[[1,'#stageTitle'],[2,'#stageParent'],[3,'#stageDeadline','right'],[4,'#saveStageBtn','right']]);await click('#saveStageBtn');
  await click('#saveJsonBtn');await sign('Добавлен этап закупки комплектующих',true);projectId=await page.evaluate(async()=>(await Production.backend.listProjects()).find(p=>p.orderNo==='924').projectId);
  await click('.stage-primary-btn');await stage('Сборка модуля','Сборочный участок','Производственная дирекция','2026-10-16','2026-11-30','work');await click('#saveStageBtn');await all();
  const assemblyRow=page.locator('#treeBody tr[data-stage-uid]').filter({hasText:'Сборка модуля'}).first();assembly=await assemblyRow.getAttribute('data-stage-uid');await assemblyRow.locator('[data-act="edit"]').click();await click('#createChildStageBtn');await stage('Проверка блока','Служба качества','Сборочный участок','2026-11-15','2026-11-18','new');
  await shot('stage-child','#stageModal .modal',[[1,'#stageTitle'],[2,'#stageParent'],[3,'#stageExecutor','right'],[4,'#saveStageBtn','right']]);await click('#saveStageBtn');
  await click('.stage-primary-btn');await stage('Итоговая приёмка','Служба качества','Производственная дирекция','2026-12-01','2026-12-31','new');await click('#saveStageBtn');await click('#saveJsonBtn');await sign('Добавлены сборка проверка и итоговая приёмка');await all();
  base=await page.evaluate(id=>Production.backend.loadById(id),projectId);procurement=base.stages.find(s=>s.title==='Закупка комплектующих').uid;
  await page.locator('#treeBody tr[data-stage-uid="'+procurement+'"] [data-act="edit"]').click();await fill('stageDeadline','2026-10-20');await fill('stageComment','Согласован перенос срока поставки');
  await shot('edit','#stageModal .modal',[[1,'#stageDeadline','right'],[2,'#stageComment'],[3,'#saveStageBtn','right']]);await click('#saveStageBtn');await click('#saveJsonBtn');await sign('Срок поставки перенесён с 15 на 20 октября');
  await all();await shot('deadline-flags','#registryView .table-wrap',[[1,'#treeBody tr.root','left'],[2,'#treeBody tr[data-stage-uid="'+procurement+'"] .status-badge','right'],[3,'#treeBody .deadline-warning','right']]);
  await click('#toggleStructureBtn');await shot('structure','#structureCard',[[1,'#toggleStructureBtn','right'],[2,'#structureTree','left']]);
  await page.locator('#treeBody tr[data-stage-uid="'+assembly+'"] [data-act="edit"]').click();await fill('stageStatus','done');await fill('completionScope','stage');
  await shot('completion-start','#stageModal .modal',[[1,'#stageStatus'],[2,'#completionScope'],[3,'#saveStageBtn','right']]);await click('#saveStageBtn');await page.waitForSelector('#saveConfirmationModal.open');
  for(const [id,value]of Object.entries({saveSigner:manager.id,saveSignerPin:pin,saveChangeComment:'Подтверждено выполнение сборки',evidenceType:'Акт приёмки',evidenceReference:'Акт № 15 от 12.11.2026',evidenceComment:'Сборка модуля завершена. Проверка блока ещё не завершена.'}))await fill(id,value);
  await shot('completion-evidence','#saveConfirmationModal .modal',[[1,'#completionTargetSummary'],[2,'#saveSigner'],[3,'#saveSignerPin','right'],[4,'#saveChangeComment'],[5,'#evidenceType'],[6,'#evidenceReference','right'],[7,'#evidenceComment'],[8,'#confirmSignedSaveBtn','right']]);await click('#confirmSignedSaveBtn');await page.waitForSelector('#saveConfirmationModal.open',{state:'hidden'});
  await all();const row='#treeBody tr[data-stage-uid="'+assembly+'"]';await shot('completed-filter',["#completionFilter",'#registryView .table-wrap'],[[1,'#completionFilter','right'],[2,row+' .status-badge','right']]);
  await shot('copy-row',row,[[1,row+' [data-act="copy"]','top']]);await click(row+' [data-act="copy"]');await fill('stageTitle','Сборка модуля - партия 13 ноября');await fill('stageStart','2026-11-13');await fill('stageDeadline','2026-11-13');
  await shot('copy-editor','#stageModal .modal',[[2,'#stageDeadline','right'],[3,'#saveStageBtn','right']]);await close('stageModal');
  await fill('projectsSearch','924');await shot('search-form','#projectSearchForm',[[1,'#projectsSearch'],[2,'#searchProjectsBtn','right']]);await click('#searchProjectsBtn');await page.waitForSelector('[data-open-project-id="'+projectId+'"]');
  await shot('search-list','#projectsModal .modal',[[3,'[data-open-project-id="'+projectId+'"]','right']]);await click('[data-open-project-id="'+projectId+'"]');await all();await fill('projectsSearch','');
  await page.evaluate(()=>document.querySelector('.more-actions').open=true);await shot('template-menu',['.more-actions > summary','.more-actions-menu'],[[1,'#downloadImportTemplateBtn','left']]);await click('#downloadImportTemplateBtn');await page.waitForFunction(()=>document.getElementById('messageArea').textContent.includes('Образец Excel сохранён'));
  const template=await page.evaluate(()=>Production.backend.excelTemplate());await more('importBtn');await page.locator('#importExcelFile').setInputFiles({name:template.fileName,mimeType:'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',buffer:Buffer.from(template.base64,'base64')});await page.waitForFunction(()=>!document.getElementById('confirmImportBtn').disabled);
  await shot('excel-import','#importModal .modal',[[2,'#importExcelName'],[3,'#importValidation'],[4,'#confirmImportBtn','right']]);await close('importModal');
  await click('#deadlinesTab');await page.waitForSelector('.deadline-row');await shot('deadlines','.deadlines-panel',[[1,'#deadlineSearch'],[2,'#refreshDeadlinesBtn','right'],[3,'.deadline-row [data-risk-project]','right']]);
  await click('#ganttTab');await fill('ganttCompletionFilter','all');await fill('ganttScale','quarter');await shot('gantt','.gantt-panel',[[1,'#ganttCompletionFilter'],[2,'#ganttScale','right'],[3,'.gantt-bar','left']]);await shot('gantt-buttons','#ganttView .panel-head',[[1,'#printGanttA4Btn','top'],[2,'#printGanttA3Btn','top']]);
  await click('#registryTab');await page.evaluate(()=>{window.__rawNativeInvoke=window.__TAURI__.core.invoke;window.__printCalls=0;window.__TAURI__.core={...window.__TAURI__.core,invoke:(command,args)=>command==='print_report'?(window.__printCalls++,Promise.resolve(true)):window.__rawNativeInvoke(command,args)}});
  await page.evaluate(()=>document.querySelector('.more-actions').open=true);await shot('print-menu',['.more-actions > summary','.more-actions-menu'],[[1,'.more-actions > summary','top'],[2,'#pdfReportBtn','left']]);await click('#pdfReportBtn');await page.waitForFunction(()=>window.__printCalls===1);
  const landscape=await page.addStyleTag({content:'@media print{@page{size:A4 landscape;margin:8mm}}'});await page.pdf({path:path.join(out,'report.pdf'),preferCSSPageSize:true,printBackground:true});await landscape.evaluate(e=>e.remove());
  await click('#printStructureBtn');await page.waitForFunction(()=>window.__printCalls===2);await page.pdf({path:path.join(out,'structure-print.pdf'),preferCSSPageSize:true,printBackground:true});
  await shot('logout',['.appbar'],[[1,'#saveState','left'],[2,'#networkLogoutBtn','right']]);
  await click('#auditBtn');await page.waitForSelector('#adminLoginModal.open');await fill('adminLoginPin',pin);await click('#confirmAdminLoginBtn');await page.waitForFunction(()=>document.getElementById('auditEvents').textContent.includes('перенесён'));
  const details=page.locator('#auditEvents .audit-event').filter({hasText:'перенесён'}).first().locator('details');if(await details.count())await details.evaluate(e=>e.open=true);
  const event=page.locator('#auditEvents .audit-event').filter({hasText:'перенесён'}).first();await event.evaluate(e=>e.id='guideJournalEvent');await shot('journal',['.admin-page-head','.admin-tabs','#guideJournalEvent'],[[1,'#auditJournalTab','top'],[2,'#guideJournalEvent','left']]);
  await click('#auditVerificationTab');await fill('verifyAdminPin',pin);await click('#verifyAuditBtn');await page.waitForFunction(()=>document.getElementById('auditVerification').textContent.includes('Проверка пройдена'));await shot('verify','#auditVerificationPanel',[[1,'#verifyAdminPin'],[2,'#verifyAuditBtn','right']]);
  await click('#auditBackupsTab');await fill('backupAdminPin',pin);await click('#createBackupBtn');await page.waitForFunction(()=>document.getElementById('backupResult').textContent.includes('Копия создана'));
  await page.locator('#backupList .backup-row').evaluateAll(rows=>rows.slice(2).forEach(row=>row.style.display='none'));await shot('backups','#auditBackupsPanel',[[1,'#backupAdminPin'],[2,'#createBackupBtn','right'],[3,'#backupList [data-restore-backup]','right']]);await click('#closeAdminBtn');
  await click('.stage-primary-btn');await click('#saveStageBtn');await shot('validation','#stageModal .modal',[[1,'#stageFormError'],[2,'#stageTitle']]);await close('stageModal');
  await page.evaluate(async({id,user,pin})=>{const p=await Production.backend.loadById(id);p.stages[0].comment='Сведения уточнены в другом окне';await Production.backend.saveSigned(p,{userId:user.id,pin,comment:'Уточнение комментария в другом окне'})},{id:projectId,user:reviewer,pin});
  await fill('projectName','Изготовление модуля А уточнение');await click('#saveJsonBtn');await page.waitForSelector('#saveConfirmationModal.open');await fill('saveSigner',manager.id);await fill('saveSignerPin',pin);await fill('saveChangeComment','Уточнено название проекта');await click('#confirmSignedSaveBtn');await page.waitForFunction(()=>document.getElementById('saveConfirmationError').textContent.includes('Проект изменён'));
  await shot('conflict','#saveConfirmationModal .modal',[[1,'#saveConfirmationError'],[2,'#saveConfirmationModal .modal-foot [data-close="saveConfirmationModal"]','right']]);
  const final=await page.evaluate(id=>Production.backend.loadById(id),projectId),report=await page.evaluate(p=>Production.backend.report(p),final);assert.equal(final.stages.length,4);assert.equal(report.donePercent,25);assert.ok(report.rows.find(r=>r.stage.uid===assembly).completionConfirmation.signatureValid);
  fs.writeFileSync(path.join(out,'capture-summary.json'),JSON.stringify({commit:'3226084e53862d163cfeaf203a9a0dd7ca6f26af',releaseSHA256:'408609e0876b8a2b83f76d6685dd64f83a1879188e97332782cf3787b6ecc13f',architecture:'released local EXE and native IPC',browser:browser.version(),captures:captures.length,projectId,stages:4,donePercent:25,signatureValid:true},null,2));
 }catch(e){fs.writeFileSync(path.join(out,'error.txt'),String(e.stack||e));if(page)await page.screenshot({path:path.join(out,'failure.png'),fullPage:true}).catch(()=>{});throw e;}
 finally{if(browser)await browser.close().catch(()=>{});if(child?.pid)spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{stdio:'ignore'});}
})().catch(e=>{console.error(e);process.exitCode=1});
