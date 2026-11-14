(async function(){
 var C=window.__reportFixture,result={error:null,documentMode:document.documentMode,printCalls:0};
 function el(id){return document.getElementById(id);}
 function pause(ms){return new Promise(function(resolve){setTimeout(resolve,ms);});}
 async function wait(f,label){var end=Date.now()+45000;while(Date.now()<end){if(f())return;await pause(100);}throw Error('Timeout '+label+': '+el('messageArea').textContent);}
 function assert(ok,message){if(!ok)throw Error(message);}
 window.confirm=function(){return true;};
 try{
  await wait(function(){return el('networkLoginUser').options.length===1;},'login');el('networkLoginUser').value=String(window.__reportUserId);el('networkLoginPin').value='739201';el('networkLoginBtn').click();
  await wait(function(){return el('onboardingModal').classList.contains('open');},'first rules');document.querySelector('[data-close="onboardingModal"]').click();
  var blob=new Blob([JSON.stringify(C)],{type:'application/json'});Object.defineProperty(blob,'name',{value:'report-fixture.json'});Object.defineProperty(el('jsonFile'),'files',{configurable:true,value:[blob]});el('jsonFile').dispatchEvent(new Event('change',{bubbles:true}));
  await wait(function(){return el('orderNo').value==='167'&&el('taskCounter').textContent.indexOf(String(C.stages.length))>=0;},'fixture import');
  window.print=function(){result.printCalls++;if(document.documentMode)window.external.PrintCurrentDocument();};
  document.querySelector('.more-actions').open=true;el('pdfReportBtn').click();await wait(function(){return result.printCalls===1;},'report print');
  var report=el('pdfReport'),rows=report.querySelectorAll('.report-stage-table tbody tr');assert(rows.length===C.stages.length,'Missing report rows');
  result.rows=rows.length;result.kpis=Array.prototype.map.call(report.querySelectorAll('.report-kpi .n'),function(e){return e.textContent;});
  result.projectFields=report.querySelectorAll('.report-field').length;result.bodyFont=getComputedStyle(report.querySelector('.report-stage-table td')).fontSize;
  if(document.documentMode){await wait(function(){return window.external.PdfReady();},'actual MSHTML PDF spool output');result.actualNativePdf=true;}
 }catch(error){result.error=String(error&&error.stack||error);}
 if(document.documentMode)window.external.Report(JSON.stringify(result));return result;
}());
