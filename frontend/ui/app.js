
(() => {
'use strict';
const {OUT_HEADER,IN_HEADER,STATUS}=Production.config;
const D=Production.domain, X=Production.exchange, J=Production.projectFile;
let state={project:{name:'',orderNo:'',initiator:'',executor:'',addressees:'',start:'',deadline:''},stages:[],nextSeq:1,collapsed:{}};
let editStageUid=null, previewKind='out';
const $=id=>document.getElementById(id);
const els={projectName:$('projectName'),orderNo:$('orderNo'),initiator:$('initiator'),executor:$('executor'),projectAddressees:$('projectAddressees'),projectStart:$('projectStart'),projectDeadline:$('projectDeadline')};

function clean(v){return String(v??'').trim()}
function safeOrderToken(v){return D.orderToken(v)}
function rootId(){return safeOrderToken(state.project.orderNo)}
function stageId(s){return D.stageId(state,s)}
function uid(){return 'u'+crypto.randomUUID()}
function byUid(id){return state.stages.find(s=>s.uid===id)||null}
function childrenOf(parentUid){return D.children(state,parentUid)}
function hasChildren(id){return childrenOf(id).length>0}
function directChildCount(id){return childrenOf(id).length}
function fullDate(v){return D.fullDate(v)}
function shortDate(v){return D.shortDate(v)}
function isoToday(){return D.today()}
function daysLeft(v){return D.daysLeft(v,isoToday())}
function derivedStageVisualStatus(s){return D.visualStatus(s,isoToday())}
function rootVisualStatus(){return D.projectStatus(state,isoToday())}
function statusText(k){return k==='overdue'?'Просрочено':STATUS[k]||k}
function escapeHtml(s){return String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}[c]))}
function updateFromForm(){for(const [k,el] of Object.entries(els)){const key=k==='projectName'?'name':k==='projectAddressees'?'addressees':k==='projectStart'?'start':k==='projectDeadline'?'deadline':k;state.project[key]=clean(el.value)}render(false)}
function syncForm(){els.projectName.value=state.project.name;els.orderNo.value=state.project.orderNo;els.initiator.value=state.project.initiator;els.executor.value=state.project.executor;els.projectAddressees.value=state.project.addressees||'';els.projectStart.value=state.project.start;els.projectDeadline.value=state.project.deadline}
Object.values(els).forEach(el=>el.addEventListener('input',updateFromForm));

const primaryStageButton=document.createElement('button');primaryStageButton.className='btn primary stage-primary-btn';primaryStageButton.textContent='+ Создать этап';primaryStageButton.onclick=()=>openStageModal(null);
function placePrimaryStageButton(){const emptySlot=$('emptyStageButtonSlot'),inlineSlot=$('inlineStageButtonSlot'),wrap=document.querySelector('.table-wrap');if(state.stages.length){wrap.classList.add('has-stages');inlineSlot.appendChild(primaryStageButton)}else{wrap.classList.remove('has-stages');emptySlot.appendChild(primaryStageButton)}}

function renderStructure(){
 const box=$('structureTree');
 if(!state.project.name&&!state.project.orderNo&&!state.stages.length){box.innerHTML='<div class="structure-empty">Заполните основной проект и добавьте этапы — здесь появится наглядная схема вложенности.</div>';return}
 function nodeHtml(label,title,executor,deadline,isRoot=false){return `<div class="structure-node ${isRoot?'root-node':''}"><span class="tree-id">${escapeHtml(label)}</span><span class="tree-name">${escapeHtml(title||'Без названия')}</span>${executor?`<span class="tree-meta">${escapeHtml(executor)}</span>`:''}${deadline?`<span class="tree-meta">до ${escapeHtml(fullDate(deadline))}</span>`:''}</div>`}
 const ul=document.createElement('ul'),root=document.createElement('li');root.innerHTML=nodeHtml('Заказ № '+rootId(),state.project.name,state.project.executor,state.project.deadline,true);ul.appendChild(root);const parents=new Map([[null,root]]);
 for(const {stage:s} of D.preorder(state)){const parent=parents.get(s.parentUid||null);let children=Array.from(parent.children).find(e=>e.tagName==='UL');if(!children){children=document.createElement('ul');parent.appendChild(children)}const li=document.createElement('li');li.innerHTML=nodeHtml(stageId(s),s.title,s.executor,s.deadline);children.appendChild(li);parents.set(s.uid,li)}box.replaceChildren(ul);
}

function render(showMessage=false){
 updateSavedState();
 $('rootId').textContent='Заказ № '+rootId();renderStructure();$('taskCounter').textContent=`${state.stages.length+1} элемент(ов)`;
 const counts={work:0,done:0,overdue:0,new:0,hold:0};state.stages.forEach(s=>counts[derivedStageVisualStatus(s)]++);
 $('sumWork').textContent=counts.work;$('sumDone').textContent=counts.done;$('sumOverdue').textContent=counts.overdue;$('sumNew').textContent=counts.new;$('sumHold').textContent=counts.hold;
 const body=$('treeBody');body.innerHTML='';
 if(state.project.name||state.project.orderNo||state.stages.length){body.appendChild(makeRootRow());}
 let visible=0;
 const hidden=new Set();for(const {stage:s,depth} of D.preorder(state)){if(state.collapsed.__root||hidden.has(s.parentUid)){hidden.add(s.uid);continue}visible++;body.appendChild(makeStageRow(s,depth+1));if(state.collapsed[s.uid])hidden.add(s.uid)}
 $('emptyState').style.display=state.stages.length?'none':'block';placePrimaryStageButton();
 if(showMessage)message('ok','Структура обновлена.');
}
function makeRootRow(){const tr=document.createElement('tr');tr.className='root';const st=rootVisualStatus(),dl=daysLeft(state.project.deadline);tr.innerHTML=`
<td><button class="btn icon" data-act="toggleRoot">${state.collapsed.__root?'▸':'▾'}</button> <button class="btn icon" title="Добавить дочерний этап" data-act="addRoot">＋</button></td>
<td><span class="id-badge">Заказ № ${escapeHtml(rootId())}</span></td>
<td><div class="tree-cell"><span class="node-icon">З</span><div><span class="node-title">${escapeHtml(state.project.name||'Без названия')}</span><span class="subnote">Основной проект · Заказ № ${escapeHtml(state.project.orderNo||'—')}</span></div></div></td>
<td>${escapeHtml(state.project.executor||'—')}</td><td>${escapeHtml(state.project.initiator||'—')}</td>
<td><span class="status-badge ${st}"><span class="status-dot ${st}"></span>${statusText(st)}</span></td>
<td>${fullDate(state.project.start)||'—'}</td><td class="deadline ${dl!==null&&dl<0&&st!=='done'?'overdue':''}">${fullDate(state.project.deadline)||'—'}</td>
<td class="left ${dl!==null&&dl<0&&st!=='done'?'overdue':dl!==null&&dl<=7?'soon':''}">${st==='done'?'—':dl===null?'—':dl}</td>`;
 tr.querySelector('[data-act="toggleRoot"]').onclick=()=>{state.collapsed.__root=!state.collapsed.__root;render()};tr.querySelector('[data-act="addRoot"]').onclick=()=>openStageModal(null);return tr}
function makeStageRow(s,depth){const tr=document.createElement('tr');const st=derivedStageVisualStatus(s),dl=daysLeft(s.deadline),kids=hasChildren(s.uid);tr.innerHTML=`
<td><button class="btn icon" title="Добавить дочерний" data-act="add">＋</button> <button class="btn icon" title="Редактировать" data-act="edit">✎</button> <button class="btn icon" title="Удалить" data-act="delete">×</button></td>
<td><span class="id-badge">${escapeHtml(stageId(s))}</span></td>
<td><div class="tree-cell"><span class="tree-prefix">${depth>1?'│   '.repeat(Math.max(0,depth-2))+'└── ':'└── '} </span><button class="expander ${kids?'':'placeholder'}" data-act="toggle">${state.collapsed[s.uid]?'▸':'▾'}</button><span class="node-icon">Э</span><div><span class="node-title">${escapeHtml(s.title)}</span><span class="subnote">входит в: ${escapeHtml(s.parentUid?stageId(byUid(s.parentUid)):'Заказ № '+rootId())}</span></div></div></td>
<td>${escapeHtml(s.executor)}</td><td>${escapeHtml(state.project.initiator||'—')}</td><td><span class="status-badge ${st}"><span class="status-dot ${st}"></span>${statusText(st)}</span></td><td>${fullDate(s.start)}</td><td class="deadline ${dl!==null&&dl<0&&st!=='done'?'overdue':''}">${fullDate(s.deadline)}</td><td class="left ${dl!==null&&dl<0&&st!=='done'?'overdue':dl!==null&&dl<=7?'soon':''}">${st==='done'?'—':dl}</td>`;
 tr.querySelector('[data-act="add"]').onclick=()=>openStageModal(s.uid);tr.querySelector('[data-act="edit"]').onclick=()=>openStageModal(s.parentUid,s.uid);tr.querySelector('[data-act="delete"]').onclick=()=>deleteStage(s.uid);tr.querySelector('[data-act="toggle"]').onclick=()=>{state.collapsed[s.uid]=!state.collapsed[s.uid];render()};return tr}

function descendants(id){return D.descendants(state,id)}
function openStageModal(parentUid=null,editUid=null){editStageUid=editUid;const s=editUid?byUid(editUid):null;$('stageModalTitle').textContent=s?'Редактирование этапа':'Новый этап';
 const parentSel=$('stageParent');parentSel.innerHTML=`<option value="">Заказ № ${escapeHtml(rootId())} — основной проект</option>`;const blocked=editUid?descendants(editUid):new Set();if(editUid)blocked.add(editUid);
 for(const {stage:x,depth} of D.preorder(state)){if(!blocked.has(x.uid)){const o=document.createElement('option');o.value=x.uid;o.textContent='— '.repeat(depth)+stageId(x)+' · '+x.title;parentSel.appendChild(o)}}
 $('stageTitle').value=s?s.title:'';$('stageExecutor').value=s?s.executor:state.project.executor;$('stageAddressees').value=s?(s.addressees||''):(state.project.addressees||'');$('stageStart').value=s?s.start:(state.project.start||isoToday());$('stageDeadline').value=s?s.deadline:(state.project.deadline||'');$('stageStatus').value=s?s.status:'new';parentSel.value=s?(s.parentUid||''):(parentUid||'');
 const seq=s?s.seq:state.nextSeq;$('stageIdPreview').textContent=rootId()+'-'+String(seq).padStart(3,'0');openModal('stageModal')}
function saveStage(){const title=clean($('stageTitle').value),executor=clean($('stageExecutor').value),addressees=clean($('stageAddressees').value),start=$('stageStart').value,deadline=$('stageDeadline').value,status=$('stageStatus').value,parentUid=$('stageParent').value||null;if(!title||!executor||!addressees||!start||!deadline||!status){message('err','Заполните все обязательные поля этапа.');return}if(start>deadline){message('err','У этапа дата начала не может быть позже дедлайна.');return}
 if(parentUid&&(parentUid===editStageUid||(editStageUid&&descendants(editStageUid).has(parentUid))||!byUid(parentUid))){message('err','Недопустимый родитель.');return}if(!D.validDate(start)||!D.validDate(deadline)||[title,executor,addressees].some(v=>/[\t\r\n]/.test(v))){message('err','Проверьте даты и уберите TAB/переносы строк.');return}
 if(editStageUid){const s=byUid(editStageUid);Object.assign(s,{title,executor,addressees,start,deadline,status,parentUid})}else{const siblings=childrenOf(parentUid);state.stages.push({uid:uid(),seq:state.nextSeq++,sort:siblings.length+1,parentUid,title,executor,addressees,start,deadline,status})}closeModal('stageModal');render();message('ok','Этап сохранён.')}
function deleteStage(u){const s=byUid(u);if(!s)return;const kids=childrenOf(u);const msg=kids.length?`Удалить «${s.title}» и все его дочерние элементы (${kids.length}+)?`:`Удалить этап «${s.title}»?`;if(!confirm(msg))return;const del=new Set([u]);let changed=true;while(changed){changed=false;for(const x of state.stages)if(x.parentUid&&del.has(x.parentUid)&&!del.has(x.uid)){del.add(x.uid);changed=true}}state.stages=state.stages.filter(x=>!del.has(x.uid));render();message('ok',`Удалено элементов: ${del.size}.`)}

function validate(){return D.validate(state)}
function message(type,text){$('messageArea').innerHTML=`<div class="notice ${type}">${escapeHtml(text)}</div>`;setTimeout(()=>{if($('messageArea').textContent===text)$('messageArea').innerHTML=''},6500)}
function showValidation(){const v=validate();if(v.errors.length){$('messageArea').innerHTML=`<div class="notice err"><b>Проверка не пройдена:</b><br>${v.errors.map(x=>'• '+escapeHtml(x)).join('<br>')}</div>`}else if(v.warnings.length){$('messageArea').innerHTML=`<div class="notice warn"><b>Ошибок нет.</b><br>${v.warnings.map(x=>'• '+escapeHtml(x)).join('<br>')}</div>`}else{$('messageArea').innerHTML='<div class="notice ok"><b>Проверка пройдена.</b> Структура готова к формированию двух TXT.</div>'}return v}

function reportGeneratedAt(){const d=new Date(),p=n=>String(n).padStart(2,'0');return `${p(d.getDate())}.${p(d.getMonth()+1)}.${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}`}
function reportCounts(){return D.report(state,isoToday()).counts}
function reportDaysText(v,status){const d=daysLeft(v);if(status==='done')return '—';if(d===null)return '—';if(d<0)return `просрочено ${Math.abs(d)} дн.`;if(d===0)return 'сегодня';return `${d} дн.`}
function reportTreeRows(){return D.preorder(state)}
function buildPdfReport(){
 const model=D.report(state,isoToday()),p=state.project,c=model.counts,total=model.total,donePct=model.donePercent,rootStatus=model.status,projectDays=model.projectDays;
 const dueSoon=model.dueSoon;
 const risks=[];if(c.overdue)risks.push(`<strong>Просрочено этапов:</strong> ${c.overdue}`);if(dueSoon)risks.push(`<strong>Срок в ближайшие 7 дней:</strong> ${dueSoon}`);if(c.hold)risks.push(`<strong>Приостановлено:</strong> ${c.hold}`);if(model.projectOverdue)risks.push('<strong>Дедлайн проекта просрочен.</strong>');if(model.overdueHeld)risks.push('<strong>Приостановленных этапов с истёкшим сроком:</strong> '+model.overdueHeld);if(!risks.length)risks.push('Отклонений по проверяемым срокам на дату отчёта не выявлено.');
 const projectRemaining=projectDays===null?'—':projectDays<0?`просрочено ${Math.abs(projectDays)} дн.`:projectDays===0?'срок сегодня':`${projectDays} дн.`;
 const rows=reportTreeRows().map(({stage:s,depth},i)=>{const st=derivedStageVisualStatus(s),d=daysLeft(s.deadline),parent=s.parentUid?stageId(byUid(s.parentUid)):`Заказ № ${rootId()}`,rowClass=st==='overdue'?'report-overdue':st==='hold'?'report-hold':'';return `<tr class="${rowClass}"><td>${i+1}</td><td>${escapeHtml(stageId(s))}</td><td><div class="report-stage-name" style="padding-left:${depth*4}mm">${depth?'↳ ':''}${escapeHtml(s.title)}</div><div class="report-stage-parent" style="padding-left:${depth*4}mm">Родитель: ${escapeHtml(parent)}</div></td><td>${escapeHtml(s.executor)}</td><td><span class="report-status ${st}">${escapeHtml(statusText(st))}</span></td><td>${escapeHtml(fullDate(s.start))}</td><td>${escapeHtml(fullDate(s.deadline))}</td><td class="report-days ${d!==null&&d<0&&st!=='done'?'overdue':d!==null&&d<=7&&d>=0&&st!=='done'?'soon':''}">${escapeHtml(reportDaysText(s.deadline,st))}</td></tr>`}).join('');
 $('pdfReport').innerHTML=`<div class="report-header"><div><div class="report-title">Отчёт по производственному проекту</div><div class="report-subtitle">Заказ № ${escapeHtml(p.orderNo||'—')} · ${escapeHtml(p.name||'Без названия')}</div></div><div class="report-meta">Сформировано: ${reportGeneratedAt()}<br>Источник: Конструктор производственного цикла</div></div>
 <div class="report-project"><div class="report-field wide"><div class="report-field-label">Проект</div><div class="report-field-value">${escapeHtml(p.name||'—')}</div></div><div class="report-field"><div class="report-field-label">Номер заказа</div><div class="report-field-value">${escapeHtml(p.orderNo||'—')}</div></div><div class="report-field"><div class="report-field-label">Общий статус</div><div class="report-field-value">${escapeHtml(statusText(rootStatus))}</div></div><div class="report-field"><div class="report-field-label">Предприятие</div><div class="report-field-value">${escapeHtml(p.addressees||'—')}</div></div><div class="report-field"><div class="report-field-label">Инициатор</div><div class="report-field-value">${escapeHtml(p.initiator||'—')}</div></div><div class="report-field"><div class="report-field-label">Исполнитель</div><div class="report-field-value">${escapeHtml(p.executor||'—')}</div></div><div class="report-field"><div class="report-field-label">Период проекта</div><div class="report-field-value">${escapeHtml(fullDate(p.start)||'—')} - ${escapeHtml(fullDate(p.deadline)||'—')} · ${escapeHtml(projectRemaining)}</div></div></div>
 <div class="report-summary"><div class="report-kpi"><div class="n">${total}</div><div class="l">Всего этапов</div></div><div class="report-kpi"><div class="n">${c.done}</div><div class="l">Выполнено (${donePct}%)</div></div><div class="report-kpi"><div class="n">${c.work}</div><div class="l">В работе</div></div><div class="report-kpi ${c.overdue?'alert':''}"><div class="n">${c.overdue}</div><div class="l">Просрочено</div></div><div class="report-kpi"><div class="n">${c.new}</div><div class="l">Не начато</div></div><div class="report-kpi ${c.hold?'warn':''}"><div class="n">${c.hold}</div><div class="l">Приостановлено</div></div></div>
 <div class="report-risk">${risks.join(' &nbsp; · &nbsp; ')}</div>
 <div class="report-section-title">Этапы и контроль сроков</div><table class="report-stage-table"><thead><tr><th>№</th><th>ID</th><th>Этап / структура</th><th>Исполнитель</th><th>Статус</th><th>Начало</th><th>Дедлайн</th><th>Осталось</th></tr></thead><tbody>${rows||'<tr><td colspan="8">Этапы не созданы.</td></tr>'}</tbody></table>
 <div class="report-footer"><span>Отчёт отражает состояние проекта на момент формирования и рассчитывается по системной дате устройства.</span><span>Заказ № ${escapeHtml(p.orderNo||'—')}</span></div>`;
}
function printPdfReport(){const v=showValidation();if(v.errors.length)return;buildPdfReport();const oldTitle=document.title;document.title=`Отчёт Заказ ${fileSafe(state.project.orderNo)}`;const restore=()=>{document.title=oldTitle;window.removeEventListener('afterprint',restore)};window.addEventListener('afterprint',restore);setTimeout(()=>{if(window.__TAURI__)window.__TAURI__.core.invoke('print_report').catch(e=>message('err','Не удалось открыть печать: '+e));else window.print()},60)}

function exportTreeOrder(){return D.preorder(state).map(r=>r.stage)}
function exportData(){return X.exportPair(state)}
function previewText(tsv){return tsv.replace(/\t/g,' ⇥ ').replace(/\r\n/g,'\n')}
function fileSafe(v){return clean(v).replace(/[\\/:*?"<>|]+/g,'-').slice(0,80)||'проект'}
async function downloadText(name,txt,mime='text/plain;charset=utf-8'){
 if(window.__TAURI__){const path=await window.__TAURI__.core.invoke('save_file',{name,text:txt});message('ok','Файл сохранён: '+path);return true}
 const blob=new Blob(['\uFEFF'+txt],{type:mime});const a=document.createElement('a');a.href=URL.createObjectURL(blob);a.download=name;document.body.appendChild(a);a.click();setTimeout(()=>{URL.revokeObjectURL(a.href);a.remove()},1000);return false
}
function openExport(){const v=showValidation();if(v.errors.length)return;previewKind='out';document.querySelectorAll('.preview-tab').forEach(x=>x.classList.toggle('active',x.dataset.preview==='out'));const suffix=fileSafe(state.project.orderNo);$('outFileName').textContent=`Список исх. Заказ ${suffix}.txt`;$('inFileName').textContent=`Список вхд. Заказ ${suffix}.txt`;updatePreview();$('exportValidation').innerHTML=v.warnings.length?`<div class="notice warn">${v.warnings.map(escapeHtml).join('<br>')}</div>`:'';openModal('exportModal')}
function updatePreview(){const d=exportData();$('txtPreview').textContent=previewText(d[previewKind])}

let savedSnapshot=null;
function updateSavedState(){if($('saveState'))$('saveState').textContent=J.fingerprint(state)===savedSnapshot?'Проект сохранён':'Есть несохранённые изменения'}
async function saveJson(){try{const snapshot=J.fingerprint(state),payload=J.stringify(state);const confirmed=await downloadText(`Производственный цикл ${fileSafe(state.project.orderNo)}.json`,payload,'application/json;charset=utf-8');if(confirmed){savedSnapshot=snapshot;updateSavedState()}else message('ok','JSON передан браузеру для сохранения. Проверьте папку загрузок.')}catch(e){message('err','Не удалось сохранить: '+e.message)}}
function loadJsonFile(f){const r=new FileReader();r.onerror=()=>message('err','Не удалось прочитать файл.');r.onload=()=>{try{const candidate=J.parse(String(r.result));if(J.fingerprint(state)!==savedSnapshot&&(state.project.name||state.stages.length)&&!confirm('Заменить текущий проект данными из файла?'))return;state=candidate;savedSnapshot=J.fingerprint(state);syncForm();render();message('ok','Проект загружен.')}catch(e){message('err','Не удалось открыть проект: '+e.message)}};r.readAsText(f,'utf-8')}

function newProject(){if((state.project.name||state.stages.length)&&!confirm('Очистить текущий проект и начать новый?'))return;state={project:{name:'',orderNo:'',initiator:'',executor:'',addressees:'',start:'',deadline:''},stages:[],nextSeq:1,collapsed:{}};syncForm();render();message('ok','Создан новый пустой проект.')}
function loadDemo(){if((state.project.name||state.stages.length)&&!confirm('Заменить текущий проект демонстрационным?'))return;state={project:{name:'Демонстрационный заказ №924',orderNo:'924',initiator:'Производственная дирекция',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-06-01',deadline:'2026-11-30'},nextSeq:9,collapsed:{},stages:[
{uid:'d1',seq:1,sort:1,parentUid:null,title:'Модуль А',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-06-01',deadline:'2026-06-30',status:'work'},
{uid:'d2',seq:2,sort:2,parentUid:null,title:'Модуль Б',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-06-01',deadline:'2026-06-30',status:'work'},
{uid:'d3',seq:3,sort:3,parentUid:null,title:'Сборочное изделие',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-06-05',deadline:'2026-08-31',status:'work'},
{uid:'d4',seq:4,sort:1,parentUid:'d3',title:'Блок управления',executor:'Исполнитель Б',addressees:'Производственная дирекция',start:'2026-06-10',deadline:'2026-07-15',status:'done'},
{uid:'d5',seq:5,sort:2,parentUid:'d3',title:'Комплект ЗИП',executor:'Исполнитель В',addressees:'Производственная дирекция',start:'2026-06-15',deadline:'2026-08-10',status:'work'},
{uid:'d6',seq:6,sort:4,parentUid:null,title:'Блок В',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-06-10',deadline:'2026-09-15',status:'work'},
{uid:'d7',seq:7,sort:5,parentUid:null,title:'Узел Г',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-07-01',deadline:'2026-10-30',status:'new'},
{uid:'d8',seq:8,sort:6,parentUid:null,title:'Привода',executor:'Исполнитель А',addressees:'Производственная дирекция',start:'2026-07-15',deadline:'2026-11-30',status:'new'}]};syncForm();render();message('ok','Загружен демонстрационный производственный цикл.')}
function openModal(id){$(id).classList.add('open')}function closeModal(id){$(id).classList.remove('open')}
document.querySelectorAll('[data-close]').forEach(b=>b.onclick=()=>closeModal(b.dataset.close));document.querySelectorAll('.modal-backdrop').forEach(m=>m.addEventListener('mousedown',e=>{if(e.target===m)closeModal(m.id)}));
$('saveStageBtn').onclick=saveStage;$('pdfReportBtn').onclick=printPdfReport;$('validateBtn').onclick=showValidation;$('exportBtn').onclick=openExport;$('saveJsonBtn').onclick=saveJson;$('loadJsonBtn').onclick=()=>$('jsonFile').click();$('jsonFile').onchange=e=>{if(e.target.files[0])loadJsonFile(e.target.files[0]);e.target.value=''};$('newBtn').onclick=newProject;$('demoBtn').onclick=loadDemo;
document.querySelectorAll('[data-preview]').forEach(b=>b.onclick=()=>{previewKind=b.dataset.preview;document.querySelectorAll('.preview-tab').forEach(x=>x.classList.toggle('active',x===b));updatePreview()});
$('downloadOutBtn').onclick=async()=>{try{const d=exportData();await downloadText($('outFileName').textContent,d.out)}catch(e){message('err','Ошибка экспорта: '+e.message)}};$('downloadInBtn').onclick=async()=>{try{const d=exportData();await downloadText($('inFileName').textContent,d.in)}catch(e){message('err','Ошибка экспорта: '+e.message)}};

document.querySelectorAll('.tab').forEach((tab,i)=>{tab.tabIndex=0;tab.setAttribute('role','button');tab.onclick=()=>{if(i===1)openExport();else if(i===2)showValidation();else window.scrollTo({top:0,behavior:'smooth'})};tab.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();tab.click()}}});
window.addEventListener('beforeunload',e=>{if((state.project.name||state.stages.length)&&J.fingerprint(state)!==savedSnapshot){e.preventDefault();e.returnValue=''}});
document.addEventListener('keydown',e=>{if(e.key==='Escape')document.querySelectorAll('.modal-backdrop.open').forEach(m=>closeModal(m.id))});
syncForm();render();
})();
