/* Used only in the real MSHTML driver after signing into the current interface. */
window.__focusedLayout=[];
window.__focusedBeforeRole=async function(index){
 if(index)return;
 function assert(ok,message){if(!ok)throw Error(message);}
 function pause(){return new Promise(function(resolve){setTimeout(resolve,180);});}
 function rect(e){var r=e.getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.right-r.left,height:r.bottom-r.top};}
 assert(document.documentMode===11,'Not MSHTML IE11');
 assert(/\/legacy\/index\.html$/.test(location.pathname),'IE did not recover from the modern entry page');
 assert(document.documentElement.className.indexOf('legacy-ie')>=0,'Missing IE layout class');
 assert(document.querySelector('#newBtn svg'),'Icons were not hydrated; app did not finish startup');
 var widths=[1440,1024,800];
 for(var i=0;i<widths.length;i++){
  window.external.Viewport(widths[i],900);window.scrollTo(0,0);await pause();
  var fields=Array.prototype.slice.call(document.querySelectorAll('.project-grid>.field')),bounds=fields.map(rect),columns=widths[i]>1100?3:widths[i]>850?2:1;
  for(var j=0;j<columns;j++)assert(Math.abs(bounds[j].top-bounds[0].top)<3,'Project fields lost their columns at '+widths[i]);
  assert(bounds[columns].top>=bounds[0].bottom-2,'Project rows overlap at '+widths[i]);
  for(var f=0;f<fields.length;f++){
   var label=rect(fields[f].querySelector('label')),input=rect(fields[f].querySelector('input'));
   assert(label.bottom<=input.top+1,'Label overlaps project input');
   assert(input.right<=document.documentElement.clientWidth+1,'Project input outside viewport');
  }
  assert(document.documentElement.scrollWidth<=document.documentElement.clientWidth+3,'Page overflows horizontally at '+widths[i]);
  var details=document.querySelector('.more-actions'),menu=document.querySelector('.more-actions-menu'),summary=details.querySelector('summary');
  assert(!details.open&&getComputedStyle(menu).display==='none','More menu is visible while closed');
  summary.click();await pause();assert(details.open&&getComputedStyle(menu).display!=='none','More menu cannot open');
  var menuBounds=rect(menu);assert(menuBounds.left>=-2&&menuBounds.right<=document.documentElement.clientWidth+2,'More menu outside viewport');
  window.external.Capture('corrected-menu-'+widths[i]);
  summary.click();await pause();assert(!details.open&&getComputedStyle(menu).display==='none','More menu cannot close');
  window.external.Capture('corrected-project-'+widths[i]);
  window.__focusedLayout.push({width:widths[i],columns:columns,labelsClear:true,menuOpensAndCloses:true,noPageOverflow:true,fields:bounds});
 }
 window.external.Viewport(1440,1000);window.scrollTo(0,0);await pause();
};
