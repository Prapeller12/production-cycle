'use strict';
(() => {
  const D=Production.domain,{OUT_HEADER,IN_HEADER}=Production.config;
  function toTsv(rows){return rows.map(row=>row.map(v=>{const s=String(v??'');if(/[\t\r\n]/.test(s))throw Error('TAB/CR/LF в поле TXT.');return s}).join('\t')).join('\r\n')+'\r\n'}
  function exportPair(state){
    const check=D.validate(state);if(check.errors.length)throw Error(check.errors.join(' '));
    const p=state.project;
    const out=[OUT_HEADER,[D.children(state,null).length||'',D.orderToken(p.orderNo),D.shortDate(p.start),p.executor,p.name,p.initiator,D.fullDate(p.deadline),p.addressees,'']];
    const incoming=[IN_HEADER,...D.preorder(state).map(({stage:s})=>[D.children(state,s.uid).length||'',s.title,D.stageId(state,s),D.fullDate(s.start),s.executor,s.addressees,D.fullDate(s.deadline),''])];
    return{out:toTsv(out),in:toTsv(incoming)};
  }
  function rows(text,header){
    if(!text.startsWith('\uFEFF'))throw Error('Нет UTF-8 BOM.');
    text=text.slice(1);if(/(^|[^\r])\n|\r(?!\n)/.test(text)||!text.endsWith('\r\n'))throw Error('Требуется CRLF.');
    const result=text.slice(0,-2).split('\r\n').map(row=>row.split('\t'));
    if(JSON.stringify(result.shift())!==JSON.stringify(header))throw Error('Неверный заголовок TXT.');
    if(result.some(row=>row.length!==header.length))throw Error('Неверное число колонок TXT.');
    return result;
  }
  function count(s){if(s==='')return 0;if(!/^\d+$/.test(s)||!Number.isSafeInteger(Number(s)))throw Error('Некорректное количество детей.');return Number(s)}
  function isoDate(value,shortYear=false){
    const pattern=shortYear?/^(\d{2})\.(\d{2})\.(\d{2})$/:/^(\d{2})\.(\d{2})\.(\d{4})$/;
    const m=String(value||'').match(pattern);if(!m)throw Error('Неверный формат даты: '+value);
    const year=shortYear?2000+Number(m[3]):Number(m[3]),iso=`${year}-${m[2]}-${m[1]}`;
    if(!D.validDate(iso))throw Error('Несуществующая дата: '+value);return iso;
  }
  // Structural preview only: manual statuses cannot be recovered from this contract.
  // Never call the normal Registry parser and never create a status LINK.
  function previewPair(outgoing,incoming){
    const out=rows(outgoing,OUT_HEADER),input=rows(incoming,IN_HEADER);if(out.length!==1)throw Error('Требуется один корневой проект.');
    const rootId=out[0][1];if(!rootId||D.orderToken(rootId)!==rootId)throw Error('Неверный ID заказа.');
    const stack=[{id:rootId,remaining:count(out[0][0]),depth:-1}],seen=new Set([rootId]),stages=[];
    for(const row of input){while(stack.length&&!stack.at(-1).remaining)stack.pop();if(!stack.length)throw Error('Лишние строки дерева.');
      const id=row[2],suffix=id.slice(rootId.length+1),seq=Number(suffix);
      if(!id.startsWith(rootId+'-')||!Number.isSafeInteger(seq)||seq<1||String(seq).padStart(3,'0')!==suffix||seen.has(id))throw Error('Повторный ID или другой заказ.');
      seen.add(id);const parent=stack.at(-1),n=count(row[0]);parent.remaining--;
      stages.push({id,parentId:parent.id,depth:parent.depth+1,childCount:n,row:[...row],status:null});
      if(n)stack.push({id,remaining:n,depth:parent.depth+1});
    }
    while(stack.length&&!stack.at(-1).remaining)stack.pop();if(stack.length)throw Error('Недостаточно дочерних строк.');
    return{rootId,stages,manualStatusAvailable:false};
  }
  function importPair(outgoing,incoming){
    const outRows=rows(outgoing,OUT_HEADER),inRows=rows(incoming,IN_HEADER);
    if(outRows.length!==1)throw Error('В исходящем TXT должен быть один основной проект.');
    const tree=previewPair(outgoing,incoming),root=outRows[0],uidById=new Map(),siblingSort=new Map(),stages=[];
    for(const item of tree.stages){
      const row=item.row,seq=Number(item.id.slice(tree.rootId.length+1)),uid=`import-${seq}`;
      uidById.set(item.id,uid);
      const parentUid=item.parentId===tree.rootId?null:uidById.get(item.parentId);
      if(item.parentId!==tree.rootId&&!parentUid)throw Error('Родитель этапа не найден: '+item.parentId);
      const sort=(siblingSort.get(parentUid)||0)+1;siblingSort.set(parentUid,sort);
      stages.push({uid,seq,sort,parentUid,title:row[1].trim(),executor:row[4].trim(),addressees:row[5].trim(),start:isoDate(row[3]),deadline:isoDate(row[6]),status:'new'});
    }
    const state={project:{name:root[4].trim(),orderNo:root[1].trim(),initiator:root[5].trim(),executor:root[3].trim(),addressees:root[7].trim(),start:isoDate(root[2],true),deadline:isoDate(root[6])},stages,nextSeq:Math.max(0,...stages.map(s=>s.seq))+1,collapsed:{}};
    const validation=D.validate(state);if(validation.errors.length)throw Error(validation.errors.join(' '));
    return{state,preview:tree,warnings:['Статусы отсутствуют в TXT-контракте: импортированные этапы установлены в статус «Не начато».']};
  }
  Production.exchange=Object.freeze({toTsv,exportPair,previewPair,importPair});
})();
