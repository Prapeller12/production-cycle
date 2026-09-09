
(() => {
'use strict';
const {STATUS}=Production.config;
const D=Production.domain, J=Production.projectFile, B=Production.backend;
let state={project:{name:'',orderNo:'',initiator:'',executor:'',addressees:'',start:'',deadline:''},stages:[],nextSeq:1,collapsed:{}};
let editStageUid=null, previewKind='out', exportCache=null,completionFilter='active',activeView='registry',ganttScale='week',structureCollapsed=false,importFiles={out:null,in:null},importCandidate=null,importPreviewKind='out';
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

function stageMatchesFilter(s){return completionFilter==='all'||(completionFilter==='done'?s.status==='done':s.status!=='done')}
function renderStructure(){
 const box=$('structureTree');
 if(!state.project.name&&!state.project.orderNo&&!state.stages.length){box.innerHTML='<div class="structure-empty">Заполните основной проект и добавьте этапы — здесь появится схема подчинённости.</div>';return}
 function nodeHtml(label,title,executor,deadline,isRoot=false){return `<div class="structure-node ${isRoot?'root-node':''}"><span class="tree-id">${escapeHtml(label)}</span><span class="tree-name">${escapeHtml(title||'Без названия')}</span>${executor?`<span class="tree-meta">${escapeHtml(executor)}</span>`:''}${deadline?`<span class="tree-meta">до ${escapeHtml(fullDate(deadline))}</span>`:''}</div>`}
 const ul=document.createElement('ul'),root=document.createElement('li');root.innerHTML=nodeHtml('Заказ № '+rootId(),state.project.name,state.project.executor,state.project.deadline,true);ul.appendChild(root);const parents=new Map([[null,root]]);
 for(const {stage:s} of D.preorder(state)){const parent=parents.get(s.parentUid||null);if(!parent)continue;let children=Array.from(parent.children).find(e=>e.tagName==='UL');if(!children){children=document.createElement('ul');parent.appendChild(children)}const li=document.createElement('li');li.innerHTML=nodeHtml(stageId(s),s.title,s.executor,s.deadline);children.appendChild(li);parents.set(s.uid,li)}box.replaceChildren(ul);
}
function toggleStructure(){structureCollapsed=!structureCollapsed;$('structureContent').classList.toggle('collapsed',structureCollapsed);$('toggleStructureBtn').textContent=structureCollapsed?'Развернуть':'Свернуть';$('toggleStructureBtn').setAttribute('aria-expanded',String(!structureCollapsed))}

function render(showMessage=false){
 updateSavedState();
 $('taskCounter').textContent=`${state.stages.length+1} элемент(ов)`;renderStructure();
 const counts={work:0,done:0,overdue:0,new:0,hold:0};state.stages.forEach(s=>counts[derivedStageVisualStatus(s)]++);
 $('sumWork').textContent=counts.work;$('sumDone').textContent=counts.done;$('sumOverdue').textContent=counts.overdue;$('sumNew').textContent=counts.new;$('sumHold').textContent=counts.hold;
 const body=$('treeBody');body.innerHTML='';
 if(state.project.name||state.project.orderNo||state.stages.length){body.appendChild(makeRootRow());}
 let visible=0;
 const hidden=new Set();for(const {stage:s,depth} of D.preorder(state)){if(state.collapsed.__root||hidden.has(s.parentUid)){hidden.add(s.uid);continue}if(stageMatchesFilter(s)){visible++;body.appendChild(makeStageRow(s,depth+1))}if(state.collapsed[s.uid])hidden.add(s.uid)}
 $('emptyState').style.display=state.stages.length?'none':'block';placePrimaryStageButton();
 renderGantt();
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
<td><div class="tree-cell"><span class="tree-prefix" style="--depth:${Math.max(1,depth)}"></span><button class="expander ${kids?'':'placeholder'}" title="${state.collapsed[s.uid]?'Развернуть дочерние этапы':'Свернуть дочерние этапы'}" data-act="toggle">${state.collapsed[s.uid]?'›':'⌄'}</button><span class="node-icon">Э</span><div><span class="node-title">${escapeHtml(s.title)}</span><span class="connection-chip">${escapeHtml(s.parentUid?stageId(byUid(s.parentUid)):'Заказ № '+rootId())}</span></div></div></td>
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

function message(type,text){$('messageArea').innerHTML=`<div class="notice ${type}">${escapeHtml(text)}</div>`;setTimeout(()=>{if($('messageArea').textContent===text)$('messageArea').innerHTML=''},6500)}
async function showValidation(){let v;try{v=await B.validate(state)}catch(e){v={errors:['Backend: '+e],warnings:[]}}if(v.errors.length){$('messageArea').innerHTML=`<div class="notice err"><b>Проверка не пройдена:</b><br>${v.errors.map(x=>'• '+escapeHtml(x)).join('<br>')}</div>`}else if(v.warnings.length){$('messageArea').innerHTML=`<div class="notice warn"><b>Ошибок нет.</b><br>${v.warnings.map(x=>'• '+escapeHtml(x)).join('<br>')}</div>`}else{$('messageArea').innerHTML='<div class="notice ok"><b>Проверка пройдена.</b> Структура готова к формированию двух TXT.</div>'}return v}

function reportGeneratedAt(){const d=new Date(),p=n=>String(n).padStart(2,'0');return `${p(d.getDate())}.${p(d.getMonth()+1)}.${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}`}
function reportDaysText(v,status){const d=daysLeft(v);if(status==='done')return '—';if(d===null)return '—';if(d<0)return `просрочено ${Math.abs(d)} дн.`;if(d===0)return 'сегодня';return `${d} дн.`}
function buildPdfReport(model){
 const p=state.project,c=model.counts,total=model.total,donePct=model.donePercent,rootStatus=model.status,projectDays=model.projectDays;
 const dueSoon=model.dueSoon;
 const risks=[];if(c.overdue)risks.push(`<strong>Просрочено этапов:</strong> ${c.overdue}`);if(dueSoon)risks.push(`<strong>Срок в ближайшие 7 дней:</strong> ${dueSoon}`);if(c.hold)risks.push(`<strong>Приостановлено:</strong> ${c.hold}`);if(model.projectOverdue)risks.push('<strong>Дедлайн проекта просрочен.</strong>');if(model.overdueHeld)risks.push('<strong>Приостановленных этапов с истёкшим сроком:</strong> '+model.overdueHeld);if(!risks.length)risks.push('Отклонений по проверяемым срокам на дату отчёта не выявлено.');
 const projectRemaining=projectDays===null?'—':projectDays<0?`просрочено ${Math.abs(projectDays)} дн.`:projectDays===0?'срок сегодня':`${projectDays} дн.`;
 const rows=model.rows.map(({stage:s,depth,visualStatus,daysRemaining},i)=>{const st=visualStatus||derivedStageVisualStatus(s),d=Number.isInteger(daysRemaining)?daysRemaining:daysLeft(s.deadline),parent=s.parentUid?stageId(byUid(s.parentUid)):`Заказ № ${rootId()}`,rowClass=st==='overdue'?'report-overdue':st==='hold'?'report-hold':'';return `<tr class="${rowClass}"><td>${i+1}</td><td>${escapeHtml(stageId(s))}</td><td><div class="report-stage-name" style="padding-left:${depth*4}mm">${depth?'↳ ':''}${escapeHtml(s.title)}</div><div class="report-stage-parent" style="padding-left:${depth*4}mm">Родитель: ${escapeHtml(parent)}</div></td><td>${escapeHtml(s.executor)}</td><td><span class="report-status ${st}">${escapeHtml(statusText(st))}</span></td><td>${escapeHtml(fullDate(s.start))}</td><td>${escapeHtml(fullDate(s.deadline))}</td><td class="report-days ${d!==null&&d<0&&st!=='done'?'overdue':d!==null&&d<=7&&d>=0&&st!=='done'?'soon':''}">${escapeHtml(reportDaysText(s.deadline,st))}</td></tr>`}).join('');
 $('pdfReport').innerHTML=`<div class="report-header"><div><div class="report-title">Отчёт по производственному проекту</div><div class="report-subtitle">Заказ № ${escapeHtml(p.orderNo||'—')} · ${escapeHtml(p.name||'Без названия')}</div></div><div class="report-meta">Сформировано: ${reportGeneratedAt()}<br>Источник: Конструктор производственного цикла</div></div>
 <div class="report-project"><div class="report-field wide"><div class="report-field-label">Проект</div><div class="report-field-value">${escapeHtml(p.name||'—')}</div></div><div class="report-field"><div class="report-field-label">Номер заказа</div><div class="report-field-value">${escapeHtml(p.orderNo||'—')}</div></div><div class="report-field"><div class="report-field-label">Общий статус</div><div class="report-field-value">${escapeHtml(statusText(rootStatus))}</div></div><div class="report-field"><div class="report-field-label">Предприятие</div><div class="report-field-value">${escapeHtml(p.addressees||'—')}</div></div><div class="report-field"><div class="report-field-label">Инициатор</div><div class="report-field-value">${escapeHtml(p.initiator||'—')}</div></div><div class="report-field"><div class="report-field-label">Исполнитель</div><div class="report-field-value">${escapeHtml(p.executor||'—')}</div></div><div class="report-field"><div class="report-field-label">Период проекта</div><div class="report-field-value">${escapeHtml(fullDate(p.start)||'—')} - ${escapeHtml(fullDate(p.deadline)||'—')} · ${escapeHtml(projectRemaining)}</div></div></div>
 <div class="report-summary"><div class="report-kpi"><div class="n">${total}</div><div class="l">Всего этапов</div></div><div class="report-kpi"><div class="n">${c.done}</div><div class="l">Выполнено (${donePct}%)</div></div><div class="report-kpi"><div class="n">${c.work}</div><div class="l">В работе</div></div><div class="report-kpi ${c.overdue?'alert':''}"><div class="n">${c.overdue}</div><div class="l">Просрочено</div></div><div class="report-kpi"><div class="n">${c.new}</div><div class="l">Не начато</div></div><div class="report-kpi ${c.hold?'warn':''}"><div class="n">${c.hold}</div><div class="l">Приостановлено</div></div></div>
 <div class="report-risk">${risks.join(' &nbsp; · &nbsp; ')}</div>
 <div class="report-section-title">Этапы и контроль сроков</div><table class="report-stage-table"><thead><tr><th>№</th><th>ID</th><th>Этап / структура</th><th>Исполнитель</th><th>Статус</th><th>Начало</th><th>Дедлайн</th><th>Осталось</th></tr></thead><tbody>${rows||'<tr><td colspan="8">Этапы не созданы.</td></tr>'}</tbody></table>
 <div class="report-footer"><span>Отчёт отражает состояние проекта на момент формирования и рассчитывается по системной дате устройства.</span><span>Заказ № ${escapeHtml(p.orderNo||'—')}</span></div>`;
}
async function printPdfReport(){const v=await showValidation();if(v.errors.length)return;try{buildPdfReport(await B.report(state));const oldTitle=document.title;document.title=`Отчёт Заказ ${fileSafe(state.project.orderNo)}`;const restore=()=>{document.title=oldTitle;window.removeEventListener('afterprint',restore)};window.addEventListener('afterprint',restore);setTimeout(()=>{if(window.__TAURI__)window.__TAURI__.core.invoke('print_report').catch(e=>message('err','Не удалось открыть печать: '+e));else window.print()},60)}catch(e){message('err','Не удалось сформировать отчёт: '+e)}}

async function exportData(){return B.exportPair(state)}
function previewText(tsv){return tsv.replace(/\t/g,' ⇥ ').replace(/\r\n/g,'\n')}
function fileSafe(v){return clean(v).replace(/[\\/:*?"<>|]+/g,'-').slice(0,80)||'проект'}
async function downloadText(name,txt,mime='text/plain;charset=utf-8'){
 if(window.__TAURI__){const path=await window.__TAURI__.core.invoke('save_file',{name,text:txt});message('ok','Файл сохранён: '+path);return true}
 const blob=new Blob(['\uFEFF'+txt],{type:mime});const a=document.createElement('a');a.href=URL.createObjectURL(blob);a.download=name;document.body.appendChild(a);a.click();setTimeout(()=>{URL.revokeObjectURL(a.href);a.remove()},1000);return false
}
async function openExport(){const v=await showValidation();if(v.errors.length)return;try{exportCache=await exportData();previewKind='out';document.querySelectorAll('.preview-tab').forEach(x=>x.classList.toggle('active',x.dataset.preview==='out'));const suffix=fileSafe(state.project.orderNo);$('outFileName').textContent=exportCache.outFileName||`Список исх. Заказ ${suffix}.txt`;$('inFileName').textContent=exportCache.inFileName||`Список вхд. Заказ ${suffix}.txt`;updatePreview();$('exportValidation').innerHTML=v.warnings.length?`<div class="notice warn">${v.warnings.map(escapeHtml).join('<br>')}</div>`:'';openModal('exportModal')}catch(e){message('err','Ошибка формирования TXT: '+e)}}
function updatePreview(){if(exportCache)$('txtPreview').textContent=previewText(exportCache[previewKind])}

let savedSnapshot=null;
function updateSavedState(){if($('saveState'))$('saveState').textContent=J.fingerprint(state)===savedSnapshot?'Проект сохранён':'Есть несохранённые изменения'}
async function saveProject(){try{const snapshot=J.fingerprint(state);if(B.native()){const v=await B.validate(state);if(v.errors.length){await showValidation();return}await B.save(state);savedSnapshot=snapshot;updateSavedState();message('ok','Проект сохранён в локальной базе данных.');return}const payload=J.stringify(state);const confirmed=await downloadText(`Производственный цикл ${fileSafe(state.project.orderNo)}.json`,payload,'application/json;charset=utf-8');if(confirmed){savedSnapshot=snapshot;updateSavedState()}else message('ok','JSON передан браузеру для сохранения. Проверьте папку загрузок.')}catch(e){message('err','Не удалось сохранить: '+e)}}
function loadJsonFile(f){const r=new FileReader();r.onerror=()=>message('err','Не удалось прочитать файл.');r.onload=()=>{try{const candidate=J.parse(String(r.result));if(J.fingerprint(state)!==savedSnapshot&&(state.project.name||state.stages.length)&&!confirm('Заменить текущий проект данными из файла?'))return;state=candidate;savedSnapshot=J.fingerprint(state);syncForm();render();message('ok','Проект загружен.')}catch(e){message('err','Не удалось открыть проект: '+e.message)}};r.readAsText(f,'utf-8')}
async function openProject(){if(!B.native()){$('jsonFile').click();return}const orderNo=clean(prompt('Введите номер ранее сохранённого заказа:',state.project.orderNo||''));if(!orderNo)return;try{const candidate=await B.load(orderNo);if(!candidate){message('err','Проект с таким номером заказа не найден.');return}if(J.fingerprint(state)!==savedSnapshot&&(state.project.name||state.stages.length)&&!confirm('Заменить текущий проект данными из локальной базы?'))return;state=candidate;savedSnapshot=J.fingerprint(state);syncForm();render();message('ok','Проект загружен из локальной базы данных.')}catch(e){message('err','Не удалось открыть проект: '+e)}}

async function loadProjectByOrder(orderNo){try{const candidate=await B.load(orderNo);if(!candidate){message('err','Проект не найден.');return}if(J.fingerprint(state)!==savedSnapshot&&(state.project.name||state.stages.length)&&!confirm('Заменить текущий проект данными из локальной базы?'))return;state=candidate;savedSnapshot=J.fingerprint(state);syncForm();closeModal('projectsModal');render();message('ok',`Открыт проект: заказ № ${orderNo}.`)}catch(e){message('err','Не удалось открыть проект: '+e)}}
async function showProjects(){
 if(!B.native()){$('jsonFile').click();return}
 openModal('projectsModal');$('projectsList').innerHTML='<div class="projects-empty">Загрузка…</div>';
 try{const items=await B.listProjects();if(!items.length){$('projectsList').innerHTML='<div class="projects-empty">Сохранённых проектов пока нет.</div>';return}
  $('projectsList').innerHTML=items.map(p=>`<div class="project-list-row"><div class="project-list-title"><strong>${escapeHtml(p.name)}</strong><span>Заказ № ${escapeHtml(p.orderNo)}</span></div><div><b>${p.donePercent}%</b><div class="progress-track"><div class="progress-fill" style="width:${p.donePercent}%"></div></div><div class="project-list-meta">${p.doneCount} из ${p.stageCount}</div></div><div>${escapeHtml(p.enterprise)}</div><div>до ${escapeHtml(fullDate(p.deadline)||'—')}</div><div class="project-list-meta">Изменён<br>${escapeHtml(String(p.updatedAt||'').replace('T',' ').slice(0,16))}</div><button class="btn primary small" data-open-order="${escapeHtml(p.orderNo)}">Открыть</button></div>`).join('');
  $('projectsList').querySelectorAll('[data-open-order]').forEach(b=>b.onclick=()=>loadProjectByOrder(b.dataset.openOrder));
 }catch(e){$('projectsList').innerHTML=`<div class="notice err">Не удалось получить список: ${escapeHtml(e)}</div>`}
}

function readTxtFile(file){return new Promise((resolve,reject)=>{const r=new FileReader();r.onerror=()=>reject(Error('Не удалось прочитать файл.'));r.onload=()=>{try{const bytes=new Uint8Array(r.result);if(bytes.length<3||bytes[0]!==0xef||bytes[1]!==0xbb||bytes[2]!==0xbf)throw Error('Файл должен быть UTF-8 с BOM.');resolve('\uFEFF'+new TextDecoder('utf-8',{fatal:true}).decode(bytes.slice(3)))}catch(e){reject(e)}};r.readAsArrayBuffer(file)})}
async function updateImportPreview(){
 importCandidate=null;$('confirmImportBtn').disabled=true;
 if(!importFiles.out||!importFiles.in){$('importPreview').textContent='Выберите оба файла для проверки.';$('importValidation').innerHTML='';return}
 try{const [outText,inText]=await Promise.all([readTxtFile(importFiles.out),readTxtFile(importFiles.in)]);const result=Production.exchange.importPair(outText,inText);importCandidate={...result,outText,inText};$('confirmImportBtn').disabled=false;$('importValidation').innerHTML=`<div class="notice ok"><b>Проверка пройдена.</b> Заказ № ${escapeHtml(result.state.project.orderNo)}, этапов: ${result.state.stages.length}.<br>${escapeHtml(result.warnings[0])}</div>`;$('importPreview').textContent=previewText(importPreviewKind==='out'?outText:inText)
 }catch(e){$('importValidation').innerHTML=`<div class="notice err"><b>Импорт невозможен:</b> ${escapeHtml(e.message||e)}</div>`;$('importPreview').textContent='Исправьте файлы и выберите их повторно.'}
}
function openImport(){importFiles={out:null,in:null};importCandidate=null;$('importOutFile').value='';$('importInFile').value='';$('importOutName').textContent='Выберите TXT (9 колонок)';$('importInName').textContent='Выберите TXT (8 колонок)';$('confirmImportBtn').disabled=true;$('importValidation').innerHTML='';$('importPreview').textContent='Выберите оба файла для проверки.';openModal('importModal')}
async function confirmImport(){if(!importCandidate)return;const candidate=importCandidate.state;if(J.fingerprint(state)!==savedSnapshot&&(state.project.name||state.stages.length)&&!confirm('Заменить текущий проект импортированными данными?'))return;try{if(B.native())await B.save(candidate);state=candidate;savedSnapshot=B.native()?J.fingerprint(state):null;syncForm();closeModal('importModal');render();message('ok',`Импортирован и сохранён заказ № ${state.project.orderNo}. Статусы этапов установлены «Не начато».`)}catch(e){message('err','Не удалось сохранить импортированный проект: '+e)}}

function renderGantt(){
 const box=$('ganttChart'),items=D.preorder(state).filter(x=>stageMatchesFilter(x.stage));
 if(!items.length){box.innerHTML='<div class="gantt-empty">Нет этапов для выбранного фильтра.</div>';$('ganttPeriod').textContent='';return}
 const dates=items.flatMap(x=>[x.stage.start,x.stage.deadline]).filter(D.validDate).map(Date.parse);if(!dates.length){box.innerHTML='<div class="gantt-empty">Для диаграммы нужны корректные даты.</div>';return}
 const day=86400000,rawStart=Math.min(...dates),rawEnd=Math.max(...dates),asDate=ms=>new Date(ms),utc=(y,m,d)=>Date.UTC(y,m,d),monthNames=['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
 let start=rawStart,end=rawEnd,tickDates=[],timelineWidth=900;
 if(ganttScale==='day'){for(let t=start;t<=end;t+=day)tickDates.push(t);timelineWidth=Math.max(900,tickDates.length*34)}
 else if(ganttScale==='week'){const d=asDate(start),shift=(d.getUTCDay()+6)%7;start-=shift*day;end+=((7-((asDate(end).getUTCDay()+6)%7)-1)%7)*day;for(let t=start;t<=end;t+=7*day)tickDates.push(t);timelineWidth=Math.max(900,tickDates.length*112)}
 else{let d=asDate(start);start=utc(d.getUTCFullYear(),d.getUTCMonth(),1);d=asDate(end);end=utc(d.getUTCFullYear(),d.getUTCMonth()+1,1)-day;for(let t=start;t<=end;){tickDates.push(t);const x=asDate(t);t=utc(x.getUTCFullYear(),x.getUTCMonth()+1,1)}timelineWidth=Math.max(900,tickDates.length*150)}
 const span=Math.max(1,Math.round((end-start)/day)+1),pctMs=ms=>Math.max(0,Math.min(100,(ms-start)/day/span*100)),pct=v=>pctMs(Date.parse(v)),format=ms=>fullDate(asDate(ms).toISOString().slice(0,10));
 const tickLabel=ms=>{const d=asDate(ms);if(ganttScale==='day')return `${String(d.getUTCDate()).padStart(2,'0')}.${String(d.getUTCMonth()+1).padStart(2,'0')}`;if(ganttScale==='week')return `${format(ms)} — ${format(Math.min(ms+6*day,end))}`;return `${monthNames[d.getUTCMonth()]} ${d.getUTCFullYear()}`};
 const ticks=tickDates.map(ms=>`<span class="gantt-tick" style="left:${pctMs(ms)}%">${tickLabel(ms)}</span>`).join(''),gridlines=tickDates.map(ms=>`<span class="gantt-gridline" style="left:${pctMs(ms)}%"></span>`).join('');
 const today=Date.parse(isoToday()),todayLine=today>=start&&today<=end?`<span class="gantt-today" style="left:${pctMs(today)}%"><span>Сегодня</span></span>`:'';
 $('ganttPeriod').textContent=`${format(start)} — ${format(end)}`;let html=`<div class="gantt-grid" style="--timeline-width:${timelineWidth}px"><div class="gantt-label"><b>Этап / связь</b></div><div class="gantt-axis">${ticks}${todayLine}</div>`;
 for(const {stage:s,depth} of items){const left=pct(s.start),width=Math.max(0.7,(Math.round((Date.parse(s.deadline)-Date.parse(s.start))/day)+1)/span*100),st=derivedStageVisualStatus(s),parent=s.parentUid?stageId(byUid(s.parentUid)):'Заказ № '+rootId();html+=`<div class="gantt-label" style="padding-left:${11+depth*14}px"><div><span class="node-title">${escapeHtml(stageId(s)+' · '+s.title)}</span><small>Связь → ${escapeHtml(parent)}</small></div></div><div class="gantt-track">${gridlines}${todayLine}<span class="gantt-bar ${st}" style="left:${left}%;width:${width}%" title="${escapeHtml(fullDate(s.start)+' — '+fullDate(s.deadline))}">${escapeHtml(s.title)}</span></div>`}box.innerHTML=html+'</div>';
}
function printGantt(size){
 if(!state.stages.length){message('err','Добавьте этапы для печати диаграммы Ганта.');return}renderGantt();const scaleText={day:'по дням',week:'по неделям',month:'по месяцам'}[ganttScale],oldTitle=document.title,style=document.createElement('style');style.id='ganttPageStyle';style.textContent=`@media print{@page{size:${size} landscape;margin:9mm}}`;document.head.appendChild(style);$('ganttPrint').innerHTML=`<div class="gantt-print-header"><div><h1>Диаграмма Ганта</h1><div>Заказ № ${escapeHtml(state.project.orderNo||'—')} · ${escapeHtml(state.project.name||'Без названия')}</div></div><div class="gantt-print-meta">Формат ${size} · масштаб ${scaleText}<br>${escapeHtml($('ganttPeriod').textContent)}</div></div><div class="gantt-chart">${$('ganttChart').innerHTML}</div>`;document.body.classList.add('print-gantt');document.title=`Диаграмма Ганта Заказ ${fileSafe(state.project.orderNo)} ${size}`;const restore=()=>{document.body.classList.remove('print-gantt');$('ganttPrint').innerHTML='';style.remove();document.title=oldTitle;window.removeEventListener('afterprint',restore)};window.addEventListener('afterprint',restore);setTimeout(()=>{if(window.__TAURI__)window.__TAURI__.core.invoke('print_report').catch(e=>{restore();message('err','Не удалось открыть печать: '+e)});else window.print()},60)
}

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
$('saveStageBtn').onclick=saveStage;$('pdfReportBtn').onclick=printPdfReport;$('validateBtn').onclick=showValidation;$('exportBtn').onclick=openExport;$('saveJsonBtn').onclick=saveProject;$('loadJsonBtn').onclick=showProjects;$('importBtn').onclick=openImport;$('jsonFile').onchange=e=>{if(e.target.files[0])loadJsonFile(e.target.files[0]);e.target.value=''};$('newBtn').onclick=newProject;$('demoBtn').onclick=loadDemo;
$('completionFilter').onchange=e=>{completionFilter=e.target.value;render()};
$('toggleStructureBtn').onclick=toggleStructure;$('ganttScale').onchange=e=>{ganttScale=e.target.value;renderGantt()};$('printGanttA4Btn').onclick=()=>printGantt('A4');$('printGanttA3Btn').onclick=()=>printGantt('A3');
$('importOutFile').onchange=e=>{importFiles.out=e.target.files[0]||null;$('importOutName').textContent=importFiles.out?importFiles.out.name:'Выберите TXT (9 колонок)';updateImportPreview()};$('importInFile').onchange=e=>{importFiles.in=e.target.files[0]||null;$('importInName').textContent=importFiles.in?importFiles.in.name:'Выберите TXT (8 колонок)';updateImportPreview()};$('confirmImportBtn').onclick=confirmImport;
document.querySelectorAll('[data-import-preview]').forEach(b=>b.onclick=()=>{importPreviewKind=b.dataset.importPreview;document.querySelectorAll('[data-import-preview]').forEach(x=>x.classList.toggle('active',x===b));if(importCandidate)$('importPreview').textContent=previewText(importPreviewKind==='out'?importCandidate.outText:importCandidate.inText)});
document.querySelectorAll('[data-preview]').forEach(b=>b.onclick=()=>{previewKind=b.dataset.preview;document.querySelectorAll('.preview-tab').forEach(x=>x.classList.toggle('active',x===b));updatePreview()});
$('downloadOutBtn').onclick=async()=>{try{const d=exportCache||await exportData();await downloadText($('outFileName').textContent,d.out)}catch(e){message('err','Ошибка экспорта: '+e)}};$('downloadInBtn').onclick=async()=>{try{const d=exportCache||await exportData();await downloadText($('inFileName').textContent,d.in)}catch(e){message('err','Ошибка экспорта: '+e)}};

document.querySelectorAll('.tab').forEach(tab=>{tab.onclick=()=>{activeView=tab.dataset.view;document.querySelectorAll('.tab').forEach(x=>x.classList.toggle('active',x===tab));$('registryView').classList.toggle('active',activeView==='registry');$('ganttView').classList.toggle('active',activeView==='gantt');if(activeView==='gantt')renderGantt()}});
window.addEventListener('beforeunload',e=>{if((state.project.name||state.stages.length)&&J.fingerprint(state)!==savedSnapshot){e.preventDefault();e.returnValue=''}});
document.addEventListener('keydown',e=>{if(e.key==='Escape')document.querySelectorAll('.modal-backdrop.open').forEach(m=>closeModal(m.id))});
syncForm();render();
})();
