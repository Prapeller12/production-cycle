/* Test instrumentation only. Every shipped button must be exercised. */
(function(){
 var trace=window.__buttonTrace={expected:[],clicks:{},labels:{},errors:[]};
 function key(e){
  if(e.id)return '#'+e.id;
  if(e.hasAttribute('data-close'))return '[data-close="'+e.getAttribute('data-close')+'"]'+(e.classList.contains('icon')?'.icon':':not(.icon)');
  var names=['data-preview','data-import-preview','data-act'];
  for(var i=0;i<names.length;i++)if(e.hasAttribute(names[i]))return '['+names[i]+'="'+e.getAttribute(names[i])+'"]';
  names=['data-open-project-id','data-risk-project','data-dictionary-index','data-restore-backup','data-notice-action'];
  for(var j=0;j<names.length;j++)if(e.hasAttribute(names[j]))return '['+names[j]+']';
  if(e.classList.contains('notice-close'))return '.notice-close';
  if(e.classList.contains('stage-primary-btn'))return '.stage-primary-btn';
  if(e.tagName==='SUMMARY'){if(e.parentNode.id==='createUserDetails')return '#createUserDetails > summary';if(e.parentNode.classList.contains('more-actions'))return '.more-actions > summary';if(e.parentNode.classList.contains('audit-changes'))return '.audit-changes > summary';}
  return null;
 }
 var buttons=document.querySelectorAll('button');
 for(var i=0;i<buttons.length;i++)if(buttons[i].id||buttons[i].hasAttribute('data-close')||buttons[i].hasAttribute('data-preview')||buttons[i].hasAttribute('data-import-preview'))trace.expected.push(key(buttons[i]));
 trace.key=key;
 document.addEventListener('click',function(event){var e=event.target.closest&&event.target.closest('button,summary');if(!e||e.disabled)return;var k=key(e);if(k){trace.clicks[k]=(trace.clicks[k]||0)+1;trace.labels[k]=e.getAttribute('aria-label')||e.textContent.replace(/\s+/g,' ').trim();}},true);
 window.addEventListener('error',function(event){trace.errors.push(String(event.message));});
 window.addEventListener('unhandledrejection',function(event){trace.errors.push(String(event.reason));});
 try{localStorage.setItem('production-cycle-onboarding-v1','done');}catch(ignored){}
}());
