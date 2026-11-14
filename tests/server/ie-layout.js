/* Execute inside real MSHTML. Geometry checks complement the existing domain smoke. */
(async function(){
 var result={error:null,documentMode:document.documentMode,userAgent:navigator.userAgent,checks:[],viewports:[],dialogs:[]};
 function assert(ok,message){if(!ok)throw new Error(message);}
 function pause(ms){return new Promise(function(resolve){setTimeout(resolve,ms);});}
 async function wait(test,label){var end=Date.now()+20000;while(Date.now()<end){if(await test())return;await pause(80);}throw new Error('Timeout '+label);}
 function el(id){return document.getElementById(id);}
 function rect(e){var r=e.getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.right-r.left,height:r.bottom-r.top};}
 function visible(e){return e&&e.getClientRects().length&&e.offsetWidth&&e.offsetHeight;}
 function capture(name){window.external.Capture(name);}
 function fit(e,label){var r=rect(e);assert(r.left>=-2&&r.right<=document.documentElement.clientWidth+2,label+' outside viewport');assert(e.scrollWidth<=e.clientWidth+3,label+' text clipped horizontally');}
 try{
  assert(document.documentMode===11,'This is not IE11 standards mode');
  await wait(function(){return visible(el('newAuditUserName'));},'administrator bootstrap');
  for(var i=0;i<3;i++){
   var width=[1440,1024,800][i];window.external.Viewport(width,[1000,768,768][i]);window.scrollTo(0,0);await pause(300);
   var cards=Array.prototype.slice.call(document.querySelectorAll('.role-cards article')).map(rect);
   var name=el('newAuditUserName'),nameLabel=name.parentNode.querySelector('span'),pin=el('newAuditUserPin'),repeat=el('newAuditUserPinConfirm');
   var item={width:document.documentElement.clientWidth,roles:cards,form:rect(el('createUserDetails')),name:rect(name),nameLabel:rect(nameLabel),pin:rect(pin),repeat:rect(repeat)};
   result.viewports.push(item);capture('bootstrap-'+width);
   assert(item.width>=width-20&&item.width<=width,'Harness did not set the requested viewport');
   assert(cards.length===3,'Role cards missing');
   if(width>850){assert(Math.abs(cards[0].top-cards[1].top)<3&&Math.abs(cards[1].top-cards[2].top)<3,'Role cards must share a row');assert(Math.abs(item.pin.top-item.repeat.top)<3&&item.repeat.left>=item.pin.right+3,'PIN fields overlap or do not share a row');}
   else{assert(cards[1].top>=cards[0].bottom+4&&item.repeat.top>=item.pin.bottom+4,'Narrow form must wrap into one column');}
   assert(item.form.width<=922,'Profile panel ignored its maximum width');
   assert(item.nameLabel.bottom<=item.name.top-2,'Field label overlaps its input');
   assert(document.documentElement.scrollWidth<=document.documentElement.clientWidth+3,'Page overflows horizontally');
   var session=rect(el('adminSessionLabel')),back=rect(el('closeAdminBtn'));
   assert(back.left>=session.right+8||back.top>=session.bottom,'Administrator header text touches its button');
   fit(el('closeAdminBtn'),'Return to project button');
   fit(el('createAuditUserBtn'),'Create administrator button');
   window.scrollTo(0,Math.max(0,item.form.top-30));await pause(100);capture('profile-'+width);
   result.checks.push('bootstrap-layout-'+width);
  }
  window.external.Viewport(1440,1000);window.scrollTo(0,0);await pause(200);
  name.value='Администратор проверки вёрстки';pin.value='739201';repeat.value='739201';el('createAuditUserBtn').click();
  await wait(function(){return !el('createAuditUserBtn').disabled&&document.querySelector('#auditUserError .ok');},'administrator creation');
  await pause(500);if(el('onboardingModal').classList.contains('open'))document.querySelector('[data-close="onboardingModal"]').click();
  el('registryTab').click();await pause(150);
  var values={projectName:'Проверка формы в Internet Explorer',orderNo:'IE-11',initiator:'Администратор',executor:'Исполнитель',projectStart:'2026-11-09',projectDeadline:'2026-12-31',projectAddressees:'Предприятие'};
  Object.keys(values).forEach(function(id){el(id).value=values[id];el(id).dispatchEvent(new Event('input',{bubbles:true}));el(id).dispatchEvent(new Event('change',{bubbles:true}));});
  document.querySelector('.stage-primary-btn').click();
  await wait(function(){return el('stageModal').classList.contains('open');},'stage dialog');
  for(var j=0;j<3;j++){
   var w=[1440,1024,800][j];window.external.Viewport(w,[1000,768,768][j]);await pause(250);
   var modal=document.querySelector('#stageModal .modal'),bounds=rect(modal),foot=rect(document.querySelector('#stageModal .modal-foot')),body=document.querySelector('#stageModal .modal-body');
   result.dialogs.push({width:document.documentElement.clientWidth,height:document.documentElement.clientHeight,modal:bounds,footer:foot,body:rect(body)});
   capture('stage-'+w);
   assert(bounds.left>=-2&&bounds.right<=document.documentElement.clientWidth+2,'Stage dialog exceeds screen width');
   assert(bounds.top>=-2&&bounds.bottom<=document.documentElement.clientHeight+2,'Stage dialog exceeds screen height');
   assert(foot.bottom<=bounds.bottom+1&&foot.height>=35,'Stage action buttons not reachable');
   assert(body.clientHeight>120&&body.scrollWidth<=body.clientWidth+3,'Stage fields overflow their scroll area');
   result.checks.push('stage-layout-'+w);
  }
 }catch(error){result.error=String(error&&error.stack||error);try{capture('failure');}catch(ignored){}}
 window.external.Report(JSON.stringify(result));
}());
