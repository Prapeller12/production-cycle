(async function(){
 var result={error:null,documentMode:document.documentMode,checks:[],events:[],nativeDialogs:0,nativeDetails:''};
 var baseline=location.search.indexOf('baseline-print=1')>=0;
 function el(id){return document.getElementById(id);}
 function assert(ok,text){if(!ok)throw Error(text);}
 function pause(ms){return new Promise(function(resolve){setTimeout(resolve,ms);});}
 async function wait(test,text){var end=Date.now()+12000;while(Date.now()<end){if(test())return;await pause(80);}throw Error('Timeout: '+text);}
 window.addEventListener('error',function(event){result.events.push(String(event.message));});
 window.addEventListener('unhandledrejection',function(event){result.events.push(String(event.reason));});
 window.confirm=function(){return true;};
 try{
  await wait(function(){return el('newAuditUserName').offsetWidth>0;},'bootstrap');
  el('newAuditUserName').value='Администратор проверки печати';el('newAuditUserPin').value='739201';el('newAuditUserPinConfirm').value='739201';el('createAuditUserBtn').click();
  await wait(function(){return !el('createAuditUserBtn').disabled&&el('auditUserError').textContent.indexOf('Создан профиль')>=0;},'administrator');
  await pause(500);if(el('onboardingModal').classList.contains('open'))document.querySelector('[data-close="onboardingModal"]').click();
  el('closeAdminBtn').click();document.querySelector('.more-actions').open=true;el('demoBtn').click();
  assert(document.querySelector('#treeBody [data-act="edit"]'),'Demo not loaded');el('ganttTab').click();
  if(baseline){
   var calls=0;window.print=function(){calls++;};
   el('printGanttA4Btn').click();await pause(350);el('printGanttA3Btn').click();await pause(350);
   result.calls=calls;assert(calls===2,'Gantt print buttons did not reach window.print; '+result.events.join('; '));
  }else{
   for(var i=0;i<4;i++){
    var scale=['day','week','month','quarter'][i];el('ganttScale').value=scale;el('ganttScale').dispatchEvent(new Event('change',{bubbles:true}));
    for(var j=0;j<2;j++){
     var size=['A4','A3'][j],before=window.external.NativePrintCount();
     el('printGantt'+size+'Btn').click();
     await wait(function(){return window.external.NativePrintCount()>before;},scale+' '+size+' native Print dialog');
     await wait(function(){return !document.body.classList.contains('print-gantt');},'afterprint cleanup');
     assert(!el('activePrintPage'),'Print page settings leaked after cancellation');
     assert(!el('ganttPrint').textContent,'Print content leaked after cancellation');
     result.checks.push('native-print-'+scale+'-'+size);await pause(180);
    }
   }
   el('registryTab').click();var previous=window.external.NativePrintCount();el('printStructureBtn').click();
   await wait(function(){return window.external.NativePrintCount()>previous;},'structure native Print dialog');
   await wait(function(){return !document.body.classList.contains('print-structure');},'structure cleanup');result.checks.push('native-print-structure');
   document.querySelector('.more-actions').open=true;previous=window.external.NativePrintCount();el('pdfReportBtn').click();
   await wait(function(){return window.external.NativePrintCount()>previous;},'report native Print dialog');result.checks.push('native-print-report');
   assert(result.events.length===0,'Unhandled errors: '+result.events.join('; '));
   result.nativeDialogs=window.external.NativePrintCount();result.nativeDetails=window.external.NativePrintDetails();
   assert(result.nativeDialogs===10,'Unexpected native print dialog count');
  }
 }catch(error){result.error=String(error&&error.stack||error);try{window.external.Capture('print-failure');}catch(ignored){}}
 window.external.Report(JSON.stringify(result));
}());
