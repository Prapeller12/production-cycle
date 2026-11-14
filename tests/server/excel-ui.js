/* Same real UI, role and first-use scenarios in Edge and native MSHTML. */
(async function(){
 var result={error:null,documentMode:document.documentMode||null,checks:[]},B=window.Production.backend,admin,users;
 function el(id){return document.getElementById(id);}
 function assert(ok,text){if(!ok)throw Error(text);}
 function pause(ms){return new Promise(function(resolve){setTimeout(resolve,ms);});}
 async function wait(test,label){var end=Date.now()+20000;while(Date.now()<end){if(await test())return;await pause(60);}throw Error('Timeout '+label+'; '+el('messageArea').textContent+'; '+el('saveConfirmationError').textContent);}
 function open(id){return el(id).classList.contains('open');}
 function visible(e){return e&&e.getClientRects().length&&getComputedStyle(e).visibility!=='hidden';}
 function click(selector){var e=document.querySelector(selector);assert(visible(e)&&!e.disabled,'Unavailable '+selector);e.click();}
 function fill(id,value){el(id).value=String(value);el(id).dispatchEvent(new Event('input',{bubbles:true}));el(id).dispatchEvent(new Event('change',{bubbles:true}));}
 function more(selector){document.querySelector('.more-actions').open=true;click(selector);}
 function file(base64,name){var raw=atob(base64),bytes=new Uint8Array(raw.length);for(var i=0;i<raw.length;i++)bytes[i]=raw.charCodeAt(i);var blob=new Blob([bytes],{type:'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'});Object.defineProperty(blob,'name',{value:name});Object.defineProperty(el('importExcelFile'),'files',{configurable:true,value:[blob]});el('importExcelFile').dispatchEvent(new Event('change',{bubbles:true}));}
 async function login(user){click('#networkLogoutBtn');await wait(function(){return open('networkLoginModal')&&!el('networkLogoutBtn').disabled;},'logout');fill('networkLoginUser',user.id);fill('networkLoginPin','739201');click('#networkLoginBtn');await wait(function(){return !open('networkLoginModal');},'login');}
 window.confirm=function(){return true;};window.__excelDownloads=window.__excelDownloads||[];
 if(document.documentMode){navigator.msSaveOrOpenBlob=function(blob,name){var reader=new FileReader();reader.onload=function(){var bytes=new Uint8Array(reader.result),text='';for(var i=0;i<bytes.length;i++)text+=String.fromCharCode(bytes[i]);window.__excelDownloads.push({name:name,base64:btoa(text),transport:'native IE msSaveOrOpenBlob handler'});};reader.readAsArrayBuffer(blob);return true;};}
 try{
  result.phase='first administrator rules';await wait(function(){return visible(el('newAuditUserName'));},'administrator form');fill('newAuditUserName','Excel UI администратор');fill('newAuditUserPin','739201');fill('newAuditUserPinConfirm','739201');click('#createAuditUserBtn');await wait(function(){return open('onboardingModal');},'first administrator rules');click('[data-close="onboardingModal"]');users=await B.listAuditUsers();admin=users[0];
  assert((await B.claimOnboarding()).show===false,'First-use claim was not persisted');click('#closeAdminBtn');
  var manager=await B.createAuditUser({displayName:'Excel UI руководитель',role:'project_manager',pin:'739201',adminUserId:admin.id,adminPin:'739201'}),reviewer=await B.createAuditUser({displayName:'Excel UI проверяющий',role:'reviewer',pin:'739201',adminUserId:admin.id,adminPin:'739201'});
  result.checks.push('First administrator sees rules once');
  for(var index=0;index<2;index++){
   var user=[manager,reviewer][index];result.phase=user.role;await login(user);await wait(function(){return open('onboardingModal');},'first rules '+user.role);click('[data-close="onboardingModal"]');await pause(100);
   document.querySelector('.more-actions').open=true;assert(!visible(el('exportBtn')),'Registry export visible to '+user.role);assert(visible(el('downloadImportTemplateBtn'))&&visible(el('hintsBtn')),'Shared menu actions missing');
   var template=await B.excelTemplate(),n=window.__excelDownloads.length;click('#downloadImportTemplateBtn');await wait(function(){return window.__excelDownloads.length>n;},'template download');assert(window.__excelDownloads[n].base64===template.base64&&window.__excelDownloads[n].name===template.fileName,'Downloaded XLSX differs from approved structure');
   more('#hintsBtn');assert(open('onboardingModal'),'Hints unavailable');click('[data-close="onboardingModal"]');
   more('#importBtn');file(window.__excelMarkup,template.fileName);await wait(function(){return !el('confirmImportBtn').disabled;},'escaped preview');assert(!el('importPreview').querySelector('img'),'Workbook markup executed in preview');assert(el('importPreview').textContent.indexOf('<img')>=0,'Escaped title missing');
   file(template.base64,template.fileName);await wait(function(){return !el('confirmImportBtn').disabled&&el('importPreview').textContent.indexOf('000924-27')>=0;},'approved preview');
   if(document.documentMode){window.external.Capture('excel-preview-'+user.role.replace(/_/g,'-'));}
   click('#confirmImportBtn');await wait(function(){return open('saveConfirmationModal');},'signature');fill('saveSigner',user.id);fill('saveSignerPin','739201');fill('saveChangeComment','Импорт Excel: '+user.role);click('#confirmSignedSaveBtn');await wait(function(){return !open('saveConfirmationModal');},'signed import');assert(el('orderNo').value==='000924-27','Signed import did not update workspace');assert(document.querySelectorAll('#treeBody tr[data-stage-uid]').length===5,'Signed import lost stages');
   localStorage.clear();await login(user);await pause(650);assert(!open('onboardingModal'),'Rules repeated after storage cleared');more('#hintsBtn');assert(open('onboardingModal'),'Returning user cannot open hints');click('[data-close="onboardingModal"]');
   result.checks.push(user.role+': first-use rules, exact template download, hidden export, escaped preview, signed import, returning-user rules and manual hints');
  }
  result.phase='administrator and main search';await login(admin);await pause(650);assert(!open('onboardingModal'),'Administrator rules repeated');document.querySelector('.more-actions').open=true;assert(visible(el('exportBtn')),'Administrator export hidden');document.querySelector('.more-actions').open=false;
  assert(document.querySelector('label[for="projectsSearch"]').textContent==='поиск проектов','Search label mismatch');assert(!el('projectsModal').contains(el('projectsSearch')),'Search inside saved-project dialog');fill('projectsSearch','000924-27');click('#searchProjectsBtn');await wait(function(){return document.querySelectorAll('[data-open-project-id]').length===2;},'filtered saved projects');assert(el('projectsList').scrollHeight>=el('projectsList').clientHeight,'Saved-project scroller missing');click('[data-close="projectsModal"].icon');fill('projectsSearch','не существует');click('#searchProjectsBtn');await wait(function(){return open('projectsModal')&&!document.querySelector('[data-open-project-id]');},'empty search');click('[data-close="projectsModal"].icon');fill('projectsSearch','');
  if(document.documentMode)window.external.Capture('excel-main-search');
  result.checks.push('Administrator retains export; exact main-screen search label; saved-project results and empty search');result.downloads=window.__excelDownloads.map(function(x){return {name:x.name,bytes:atob(x.base64).length,transport:x.transport};});
 }catch(error){result.error=String(error&&error.stack||error);try{if(document.documentMode)window.external.Capture('excel-failure');}catch(ignored){}}
 if(document.documentMode)window.external.Report(JSON.stringify(result));else window.__excelUIResult=result;
 return result;
}());
