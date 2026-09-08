'use strict';
(() => {
  const D=Production.domain;
  function normalize(obj){
    if(!obj||typeof obj!=='object'||!Production.config.formats.includes(obj.format))throw Error('Неизвестный формат JSON.');
    if(!obj.project||typeof obj.project!=='object'||Array.isArray(obj.project)||!Array.isArray(obj.stages))throw Error('Отсутствуют проект или этапы.');
    const candidate={project:{...obj.project},stages:obj.stages.map(s=>({...s})),nextSeq:obj.nextSeq,collapsed:{}};
    // Legacy optional enterprise fields can be empty drafts, but must not be guessed.
    if(candidate.project.addressees===undefined)candidate.project.addressees='';
    for(const s of candidate.stages)if(s.addressees===undefined)s.addressees='';
    const check=D.validate(candidate,{draft:true});if(check.errors.length)throw Error(check.errors.join(' '));
    let max=0;for(const s of candidate.stages)max=Math.max(max,s.seq);
    if(candidate.nextSeq===undefined)candidate.nextSeq=max+1;
    if(!Number.isSafeInteger(candidate.nextSeq)||candidate.nextSeq<=max||candidate.nextSeq<1)throw Error('Счётчик nextSeq должен быть больше всех seq этапов.');
    return candidate;
  }
  function fingerprint(state){return JSON.stringify({project:state.project,stages:state.stages,nextSeq:state.nextSeq})}
  function stringify(state){const candidate=normalize({format:'production-cycle-builder-v8',...state});return JSON.stringify({format:'production-cycle-builder-v8',version:8,savedAt:new Date().toISOString(),...candidate},null,2)}
  function parse(text){return normalize(JSON.parse(text.replace(/^\uFEFF/,'')))}
  Production.projectFile=Object.freeze({parse,stringify,fingerprint});
})();
