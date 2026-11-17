(function(root){'use strict';
  const D=root.PCDomain;
  const defs=[
    {id:'participation',title:'Соисполнители заказа',hint:'Доля назначенных этапов и их выполнение',icon:'users'},
    {id:'enterprises',title:'Этапы по предприятиям',hint:'Группировка по предприятию проекта',icon:'factory'},
    {id:'portfolio',title:'Проекты и сроки',hint:'Срок проекта и выполнение этапов',icon:'folder'},
    {id:'calendar',title:'Ближайшие сроки',hint:'Незавершённые этапы по дате окончания',icon:'calendar'},
    {id:'executors',title:'Этапы по исполнителям',hint:'Количество этапов, без оценки трудоёмкости',icon:'users'},
    {id:'risks',title:'Этапы, требующие внимания',hint:'От показателя — к конкретной работе',icon:'flag'},
    {id:'statuses',title:'Статусы этапов',hint:'Статус и просрочка учитываются отдельно',icon:'chart'},
    {id:'events',title:'Изменения и подтверждения',hint:'Дата регистрации события, не дата выполнения работ',icon:'clock'},
    {id:'quality',title:'Проверка дат и планов',hint:'Несогласованные даты требуют проверки',icon:'check'}
  ];
  const paths={grid:'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z',factory:'M3 21V9l6 3V6l6 4V3h6v18H3Zm4-5h1m3 0h1m3 0h1',folder:'M3 7V5h6l2 2h10v13H3V7Z',calendar:'M4 5h16v16H4zM8 3v4m8-4v4M4 10h16',users:'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2m20 0v-2a4 4 0 0 0-3-3.87M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm8-7a4 4 0 0 1 0 7.75',flag:'M4 22V3m0 1c5-4 10 4 16 0v10c-6 4-11-4-16 0',chart:'M4 20h17M7 16V9m6 7V4m6 12v-5',clock:'M12 8v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z',check:'m5 12 4 4L19 6',settings:'M4 7h16M4 17h16M8 4v6m8 4v6',refresh:'M20 7v5h-5M4 17v-5h5m-4-4a8 8 0 0 1 13-3l2 3M4 16l2 3a8 8 0 0 0 13-3',download:'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',upload:'M12 16V4m-5 5 5-5 5 5M4 16v5h16v-5',search:'M21 21l-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z',arrow:'M5 12h14m-5-5 5 5-5 5',close:'m6 6 12 12M6 18 18 6',print:'M6 9V3h12v6M6 18H3v-8h18v8h-3M6 14h12v7H6z',help:'M9 9a3 3 0 0 1 6 0c0 2-3 2-3 4m0 4h.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z',save:'M4 3h13l4 4v14H3V3Zm3 0v6h10V3M7 21v-8h10v8'};
  const icon=name=>'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="'+(paths[name]||paths.grid)+'"/></svg>';
  const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const fmt=s=>D.validDate(s)?s.split('-').reverse().join('.'):'—';
  const number=n=>Number(n).toLocaleString('ru-RU');
  const dateTime=s=>{const d=new Date(s);return Number.isNaN(d.getTime())?'—':d.toLocaleString('ru-RU',{day:'2-digit',month:'2-digit',year:'numeric',hour:'2-digit',minute:'2-digit'});};
  const clone=value=>JSON.parse(JSON.stringify(value));
  const defaults=['participation','enterprises','portfolio','calendar','risks','statuses'];
  const baseSettings=()=>({version:2,theme:'light',font:'normal',horizon:7,countMode:'all',columns:2,widgets:defs.map(d=>({id:d.id,visible:defaults.includes(d.id),width:d.id==='participation'?'full':'half'})),legacyArchive:null});
  function cleanSettings(input){
    const raw=typeof input==='string'?JSON.parse(input):input;
    if(!raw||typeof raw!=='object'||![1,2].includes(raw.version))throw Error('Неподдерживаемый формат настроек.');
    const result=baseSettings();
    for(const [key,values] of Object.entries({theme:['light','dark','system'],font:['normal','large'],horizon:[7,14,30],countMode:['all','leaves'],columns:[1,2]}))if(values.includes(raw[key]))result[key]=raw[key];
    if(Array.isArray(raw.widgets)){
      const seen=new Set();result.widgets=[];
      for(const w of raw.widgets)if(w&&defs.some(d=>d.id===w.id)&&!seen.has(w.id)){
        seen.add(w.id);result.widgets.push({id:w.id,visible:w.visible!==false,width:w.width==='full'?'full':'half'});
      }
      for(const w of baseSettings().widgets)if(!seen.has(w.id))result.widgets.push({...w,visible:w.id==='participation'});
    }
    result.legacyArchive=raw.version===1?clone(raw):(raw.legacyArchive&&typeof raw.legacyArchive==='object'?clone(raw.legacyArchive):null);
    return result;
  }
  function mount(element,options={}){
    if(!element||typeof element.querySelector!=='function')throw Error('Не найден контейнер статистики.');
    if(element.hasAttribute('data-pcd'))throw Error('Статистика уже подключена к этому контейнеру.');
    const adapter=options.settingsAdapter,legacyKey=options.storageKey||'production-cycle-dashboards:v1';
    let settings=baseSettings(),draft=null,revision=0,destroyed=false,busy=false,saving=false,loading=true,loadFailed=false,location='local',requestId=0,leaveResolve=null,leavePromise=null,draftTimer=null,draftWrite=Promise.resolve(),draftGeneration=0;
    let data=D.normalize(options.data||{projects:[],source:{label:'Данные не загружены',asOf:D.today()}});
    let filters={orderNo:'',enterprise:'',executor:'',projectId:'',search:'',asOf:D.today()};
    const ui={modal:null,message:null,tableMode:'overdue',barMetric:'overdue',calendarDay:'',detail:null,expanded:null,returnView:null,printing:false,conflict:null,recovered:null,legacyCandidate:null,saveFailed:false,dragId:null,dragTarget:null,dragCancelled:false};
    let model,widgetContext=null;
    const current=()=>draft||settings;
    const changed=()=>!!draft&&JSON.stringify(draft)!==JSON.stringify(settings);
    const titleById=id=>defs.find(d=>d.id===id);
    const empty=(text='По выбранным условиям нет данных')=>'<div class="pc-empty">'+icon('folder')+'<span>'+esc(text)+'</span></div>';
    const badge=(text,kind='neutral')=>'<span class="pc-badge '+kind+'">'+esc(text)+'</span>';
    const openBtn=(p,s)=>'<button class="pc-icon-button" data-action="detail" data-project="'+p+'" '+(s?'data-stage="'+esc(s)+'"':'')+' aria-label="Подробнее">'+icon('arrow')+'</button>';
    const compact=()=>!ui.printing&&widgetContext&&widgetContext.width==='half'&&ui.expanded!==widgetContext.id;
    const visibleRows=rows=>compact()?rows.slice(0,3):rows;
    function previewCount(total,noun='записей'){
      if(!compact())return '';
      return '<div class="pc-preview-footer"><span>Показано '+Math.min(3,total)+' из '+number(total)+' '+noun+'</span>'+(total?'<button class="pc-button subtle" data-action="show-all" data-widget="'+widgetContext.id+'" data-focus="show-all-'+widgetContext.id+'">'+(total>3?'Показать все '+number(total):'Подробнее')+'</button>':'')+'</div>';
    }
    function notify(message,type='ok'){ui.message={message,type};}
    function sourceLabel(){return data.source.kind==='demo'?'Демонстрационные данные':data.source.kind==='host'?'Данные программы':'Данные из файла';}
    function filteredOptions(key){return [...new Set(data.projects.flatMap(p=>key==='executor'?p.stages.map(s=>s.executor):[p[key]]))].sort((a,b)=>a.localeCompare(b,'ru',{numeric:true}));}
    function selectOptions(values,chosen,all){return '<option value="">'+all+'</option>'+(!values.includes(chosen)&&chosen?'<option value="'+esc(chosen)+'" selected>'+esc(chosen)+' — нет в источнике</option>':'')+values.map(v=>'<option value="'+esc(v)+'" '+(v===chosen?'selected':'')+'>'+esc(v)+'</option>').join('');}
    function scopeLabel(){return ['Заказ: '+(filters.orderNo?'№ '+filters.orderNo:'все'), 'предприятие: '+(filters.enterprise||'все'),'исполнитель этапа: '+(filters.executor||'все'),filters.projectId?'проект ID '+filters.projectId:'',filters.search?'поиск «'+filters.search+'»':'',current().countMode==='leaves'?'только конечные этапы':'все уровни этапов','дата расчёта: '+fmt(model.asOf)].filter(Boolean).join(' · ');}
    function riskRows(){let rows=model.active;if(ui.calendarDay)rows=rows.filter(s=>s.deadline===ui.calendarDay);else if(ui.tableMode==='overdue')rows=rows.filter(s=>s.daysLeft<0);else if(ui.tableMode==='soon')rows=rows.filter(s=>s.daysLeft>=0&&s.daysLeft<=current().horizon);else if(ui.tableMode==='hold')rows=rows.filter(s=>s.status==='hold');return [...rows].sort((a,b)=>a.daysLeft-b.daysLeft||a.projectId-b.projectId||a.seq-b.seq);}
    function renderBars(groups,type){
      const metric=ui.barMetric,sorted=[...groups].sort((a,b)=>b[metric]-a[metric]||a.name.localeCompare(b.name,'ru')),max=Math.max(1,...sorted.map(g=>g[metric]));
      return '<div class="pc-bar-switch"><label>Показатель <select data-control="barMetric" data-focus="barMetric-'+widgetContext.id+'" aria-label="Показатель сравнения"><option value="overdue" '+(metric==='overdue'?'selected':'')+'>Просроченные этапы</option><option value="active" '+(metric==='active'?'selected':'')+'>Незавершённые этапы</option></select></label></div>'+(sorted.length?'<div class="pc-bars">'+visibleRows(sorted).map(g=>'<button class="pc-bar-row" data-action="filter-group" data-type="'+type+'" data-value="'+esc(g.name)+'"><span class="pc-bar-name">'+esc(g.name)+'</span><span class="pc-bar-track"><span class="pc-bar-fill '+(metric==='overdue'?'danger':'')+'" style="width:'+g[metric]/max*100+'%"></span></span><strong>'+number(g[metric])+'</strong><span class="pc-bar-meta">'+number(g.active)+' незав.</span></button>').join('')+'</div>':empty())+previewCount(sorted.length,type==='executor'?'исполнителей':'предприятий');
    }
    function portfolio(){
      const rows=[...model.projects].sort((a,b)=>Number(b.projectOverdue)-Number(a.projectOverdue)||b.overdue-a.overdue||a.deadline.localeCompare(b.deadline));
      if(!rows.length)return empty()+previewCount(0,'проектов');
      if(compact())return '<div class="pc-overview-list">'+visibleRows(rows).map(p=>'<article><div><strong>'+esc(p.name)+'</strong><small>№ '+esc(p.orderNo)+' · '+esc(p.enterprise)+'</small><span>'+fmt(p.deadline)+' · '+(p.projectDone?'Завершён':p.projectOverdue?'⚠ Срок истёк':'В работе')+'</span></div><div class="pc-overview-value"><strong>'+p.progress+'%</strong><small>'+p.done+' из '+p.total+'</small>'+openBtn(p.projectId)+'</div></article>').join('')+'</div>'+previewCount(rows.length,'проектов');
      return '<div class="pc-table-wrap"><table class="pc-table"><thead><tr><th>Проект / предприятие</th><th>Выполнено этапов</th><th>Срок проекта</th><th aria-label="Действие"></th></tr></thead><tbody>'+rows.map(p=>'<tr><td><strong>'+esc(p.name)+'</strong><small>№ '+esc(p.orderNo)+' · ID '+p.projectId+' · '+esc(p.enterprise)+'</small>'+(p.overdue?badge('⚠ '+p.overdue+' просроч.','danger'):'')+'</td><td><div class="pc-progress-label"><strong>'+p.progress+'%</strong><span>'+p.done+' из '+p.total+'</span></div><div class="pc-progress-track"><span style="width:'+p.progress+'%"></span></div></td><td>'+fmt(p.deadline)+'<small>'+(p.projectDone?'Завершён':p.projectOverdue?'<span class="pc-danger-text">⚠ Срок истёк</span>':'Осталось '+D.days(p.deadline,model.asOf)+' дн.')+'</small></td><td>'+openBtn(p.projectId)+'</td></tr>').join('')+'</tbody></table></div>';
    }
    function calendar(){
      const max=Math.max(1,...model.calendar.map(b=>b.count));
      return '<div class="pc-calendar-meta"><strong>'+number(model.counts.today+model.counts.dueSoon)+' этапов</strong><span>на дату расчёта и в следующие '+current().horizon+' дн.</span></div><div class="pc-calendar" style="--pc-days:'+model.calendar.length+'">'+model.calendar.map((b,i)=>'<button class="pc-day '+(ui.calendarDay===b.date?'selected':'')+'" data-action="calendar-day" data-date="'+b.date+'" data-focus="calendar-'+b.date+'" aria-label="'+fmt(b.date)+': '+b.count+' этапов"><strong>'+b.count+'</strong><span class="pc-day-track"><span style="height:'+Math.max(b.count?4:0,b.count/max*100)+'%"></span></span><span>'+b.date.slice(8)+'.'+b.date.slice(5,7)+'</span>'+(i===0?'<small>расчёт</small>':'')+'</button>').join('')+'</div>';
    }
    function riskTable(){
      const rows=riskRows(),modes=[['all','Все незавершённые'],['overdue','Просроченные'],['soon','Ближайшие сроки'],['hold','Приостановленные']];
      const tools='<div class="pc-risk-tools"><div class="pc-segments">'+modes.map(([key,label])=>'<button data-action="risk-mode" data-value="'+key+'" data-focus="risk-mode-'+key+'" aria-pressed="'+(!ui.calendarDay&&ui.tableMode===key)+'">'+label+'</button>').join('')+'</div>'+(ui.calendarDay?'<button class="pc-button subtle" data-action="clear-day">'+fmt(ui.calendarDay)+' ×</button>':'')+'</div>';
      if(!rows.length)return tools+empty('В этой группе этапов нет')+previewCount(0,'этапов');
      if(compact())return tools+'<div class="pc-overview-list">'+visibleRows(rows).map(s=>'<article><div><strong>'+esc(s.title)+'</strong><small>№ '+esc(s.orderNo)+' · '+esc(s.executor)+'</small><span class="'+(s.daysLeft<0?'pc-danger-text':'')+'">'+(s.daysLeft<0?'⚠ Просрочен · ':'')+fmt(s.deadline)+' · '+D.labels[s.status]+'</span></div>'+openBtn(s.projectId,s.uid)+'</article>').join('')+'</div>'+previewCount(rows.length,'этапов');
      return tools+'<div class="pc-table-wrap"><table class="pc-table"><thead><tr><th>Этап / проект</th><th>Исполнитель</th><th>Статус</th><th>Срок</th><th>Отклонение</th><th></th></tr></thead><tbody>'+rows.map(s=>'<tr><td><strong>'+esc(s.title)+'</strong><small>№ '+esc(s.orderNo)+' · '+esc(s.projectName)+'</small></td><td>'+esc(s.executor)+'</td><td>'+badge(D.labels[s.status],s.status==='hold'?'hold':'neutral')+'</td><td>'+fmt(s.deadline)+'</td><td>'+badge(s.daysLeft<0?'⚠ Просрочен на '+Math.abs(s.daysLeft)+' дн.':s.daysLeft===0?'Сегодня':s.daysLeft+' дн.',s.daysLeft<0?'danger':s.daysLeft===0?'hold':'neutral')+'</td><td>'+openBtn(s.projectId,s.uid)+'</td></tr>').join('')+'</tbody></table></div><div class="pc-footnote">Всего '+number(rows.length)+' этапов</div>';
    }
    function statusWidget(){
      if(!model.rows.length)return empty();
      return '<div class="pc-status-stack">'+model.statuses.filter(s=>s.count).map(s=>'<span class="'+s.status+'" style="flex:'+s.count+'" title="'+s.label+': '+s.count+'"></span>').join('')+'</div><div class="pc-status-list">'+model.statuses.map(s=>'<div><span><i class="pc-dot '+s.status+'"></i>'+s.label+'</span><strong>'+s.count+'</strong><small>'+Math.round(s.count/model.rows.length*100)+'%</small></div>').join('')+'</div><div class="pc-status-note">⚠ Просрочено '+model.counts.overdue+' этапов, из них приостановлено '+model.counts.heldOverdue+'. Просрочка может сочетаться со статусом.</div>';
    }
    function eventWidget(){
      const note='<div class="pc-footnote">Фильтр исполнителя этапа не ограничивает события журнала.</div>';
      if(!data.source.auditAvailable)return empty('Журнал не передан. Подтверждения не проверялись.')+note;
      if(!model.events.length)return empty('В переданном журнале событий нет.')+note+previewCount(0,'событий');
      return '<div class="pc-events">'+visibleRows(model.events).map(e=>'<article class="pc-event"><span class="pc-event-dot '+(e.eventType==='stage_completed'?'success':'')+'"></span><div><strong>'+esc(({stage_completed:'Подтверждено завершение этапа',project_created:'Создан проект',project_changed:'Изменён проект',dictionary_replaced:'Изменён справочник'})[e.eventType]||e.eventType)+'</strong><small>'+esc(e.actorName)+' · '+dateTime(e.createdAt)+' · ID '+e.projectId+'</small>'+(compact()?'<p>'+esc(e.comment.length>120?e.comment.slice(0,120)+'…':e.comment)+'</p>':'<p>'+esc(e.comment)+'</p>'+(e.evidenceReference?'<small>'+esc(e.evidenceType)+': '+esc(e.evidenceReference)+'</small>':'')+(e.changes.length?'<details><summary>Что изменилось · '+e.changes.length+'</summary>'+e.changes.map(c=>'<p>'+esc(c.entity)+' '+esc(c.entityId)+' · '+esc(c.field)+'<br><del>'+esc(c.before)+'</del> → '+esc(c.after)+'</p>').join('')+'</details>':''))+'</div></article>').join('')+'</div>'+note+previewCount(model.events.length,'событий');
    }
    function quality(){return '<div class="pc-quality">'+[['emptyProjects','Проекты без этапов','План ещё не раскрыт на работы'],['afterProject','Этапы за сроком проекта','Срок этапа позже срока проекта'],['afterParent','Этапы за сроком головного','Проверка согласованности дат']].map(([key,label,hint])=>'<div><span>'+label+'<small>'+hint+'</small></span><strong class="'+(model.quality[key]?'pc-danger-text':'')+'">'+model.quality[key]+'</strong></div>').join('')+'</div><div class="pc-footnote">Это проверка дат плана. Технологические зависимости и критический путь не рассчитываются.</div>';}
    function participationWidget(){
      if(!filters.orderNo)return empty('Выберите заказ в общем фильтре сверху, чтобы увидеть соисполнителей.');
      const p=D.participation(data,filters,current(),filters.orderNo),pct=n=>n.toLocaleString('ru-RU',{maximumFractionDigits:1})+'%';
      const meta='<div class="pc-participation-toolbar"><span>Заказ № '+esc(filters.orderNo)+' · Проектов: '+p.projectCount+' · Этапов в расчёте: '+p.total+'</span><strong>Выполнено '+p.doneTotal+' из '+p.total+' · '+pct(p.donePercent)+'</strong></div>';
      const note='<div class="pc-footnote">Доли всех исполнителей выбранного состава; выбранный исполнитель выделен. Доля этапов заказа = назначено / '+p.total+'. Выполнено из назначенных = выполнено / назначено исполнителю. Расчёт по количеству, без стоимости и трудоёмкости.</div>';
      if(!p.total)return meta+empty('У выбранного заказа нет этапов по текущим условиям отбора.')+note;
      if(compact())return meta+'<div class="pc-overview-list">'+visibleRows(p.members).map(m=>'<article class="'+(m.highlighted?'pc-member-highlight':'')+'"><div><strong>'+esc(m.executor)+'</strong><div class="pc-share-label"><strong>'+pct(m.sharePercent)+'</strong><span>'+m.assigned+' из '+p.total+' этапов заказа</span></div><div class="pc-share-track"><span class="pc-share-all" style="width:'+m.sharePercent+'%"><span class="pc-share-done" style="width:'+m.donePercent+'%"></span></span></div><small>Выполнено '+m.done+' из '+m.assigned+' · '+pct(m.donePercent)+'</small></div></article>').join('')+'</div>'+previewCount(p.members.length,'исполнителей')+note;
      return meta+'<div class="pc-table-wrap"><table class="pc-table pc-participation-table"><thead><tr><th>Соисполнитель</th><th>Доля этапов заказа</th><th>Назначено</th><th>Выполнено</th><th>Выполнено из назначенных</th><th>Просрочено</th></tr></thead><tbody>'+p.members.map(m=>'<tr class="'+(m.highlighted?'pc-member-highlight':'')+'"><td><button class="pc-member-name" data-action="filter-group" data-type="executor" data-value="'+esc(m.executor)+'">'+esc(m.executor)+'</button></td><td><div class="pc-share-label"><strong>'+pct(m.sharePercent)+'</strong><span>'+m.assigned+' из '+p.total+'</span></div><div class="pc-share-track"><span class="pc-share-all" style="width:'+m.sharePercent+'%"><span class="pc-share-done" style="width:'+m.donePercent+'%"></span></span></div></td><td>'+m.assigned+'</td><td>'+m.done+'</td><td><span class="pc-member-completion">'+pct(m.donePercent)+'</span><small>'+m.done+' из '+m.assigned+'</small></td><td>'+(m.overdue?badge('⚠ '+m.overdue,'danger'):'0')+'</td></tr>').join('')+'</tbody></table></div><div class="pc-participation-legend"><span><i class="pc-share-legend-all"></i>Назначенные этапы</span><span><i class="pc-share-legend-done"></i>Выполненная часть</span></div>'+note;
    }
    const widgets={participation:participationWidget,enterprises:()=>renderBars(model.enterprises,'enterprise'),executors:()=>renderBars(model.executors,'executor'),portfolio,calendar,risks:riskTable,statuses:statusWidget,events:eventWidget,quality};
    function settingSelect(key,label,choices){return '<label>'+label+'<select data-setting="'+key+'" data-focus="setting-'+key+'" '+(saving?'disabled':'')+'>'+choices.map(([value,text])=>'<option value="'+value+'" '+(current()[key]===value?'selected':'')+'>'+text+'</option>').join('')+'</select></label>';}
    function editor(){
      if(!draft||ui.printing)return '';
      return '<aside class="pc-layout-editor" aria-label="Настройка экрана"><h2 tabindex="-1" data-focus="editor-heading">Настроить экран</h2><p class="pc-footnote">Это предпросмотр. Подтвердите изменения кнопкой «Сохранить настройки».</p><div class="pc-setting-grid">'+settingSelect('columns','Колонки',[[1,'Одна'],[2,'Две']])+settingSelect('font','Размер текста',[['normal','Обычный'],['large','Крупный']])+settingSelect('theme','Оформление',[['light','Светлое'],['dark','Тёмное'],['system','Как в системе']])+settingSelect('horizon','Ближайшие сроки',[[7,'7 дней'],[14,'14 дней'],[30,'30 дней']])+settingSelect('countMode','Считать этапы',[['all','Все, как в программе'],['leaves','Только конечные']])+'</div><div class="pc-widget-list">'+draft.widgets.map((w,i)=>'<div class="pc-widget-setting" data-drop-widget="'+w.id+'"><button class="pc-drag-handle pc-icon-button" draggable="'+!saving+'" data-drag="'+w.id+'" data-focus="drag-'+w.id+'" aria-label="Переместить: '+esc(titleById(w.id).title)+'" '+(saving?'disabled':'')+'>⠿</button><label><input type="checkbox" data-widget-toggle="'+w.id+'" data-focus="toggle-'+w.id+'" '+(w.visible?'checked':'')+' '+(saving?'disabled':'')+'>'+titleById(w.id).title+'</label><select aria-label="Ширина: '+esc(titleById(w.id).title)+'" data-widget-width="'+w.id+'" data-focus="width-'+w.id+'" '+(saving?'disabled':'')+'><option value="half" '+(w.width==='half'?'selected':'')+'>Половина строки</option><option value="full" '+(w.width==='full'?'selected':'')+'>Вся строка</option></select><button class="pc-icon-button" data-action="move-up" data-widget="'+w.id+'" data-focus="move-up-'+w.id+'" '+(i===0||saving?'disabled':'')+' aria-label="Выше: '+esc(titleById(w.id).title)+'">↑</button><button class="pc-icon-button" data-action="move-down" data-widget="'+w.id+'" data-focus="move-down-'+w.id+'" '+(i===draft.widgets.length-1||saving?'disabled':'')+' aria-label="Ниже: '+esc(titleById(w.id).title)+'">↓</button></div>').join('')+'</div><div class="pc-editor-actions"><button class="pc-button primary" data-action="save-settings" '+(saving?'disabled':'')+'>'+icon('save')+(saving?'Сохранение…':ui.saveFailed?'Повторить сохранение':'Сохранить настройки')+'</button><button class="pc-button" data-action="cancel-settings" '+(saving?'disabled':'')+'>Отмена</button><button class="pc-button subtle" data-action="reset-settings" '+(saving?'disabled':'')+'>Вернуть исходный вид</button><button class="pc-button subtle" data-action="export-settings">Экспорт черновика</button>'+(current().legacyArchive?'<button class="pc-button subtle" data-action="export-archive">Архив прежних настроек</button>':'')+'</div></aside>';
    }
    function widget(w){
      widgetContext=w;const d=titleById(w.id),body=widgets[w.id]();widgetContext=null;
      return '<section class="pc-widget '+w.width+(draft?' pc-widget-editing':'')+'" data-widget="'+w.id+'" data-drop-widget="'+w.id+'"><div class="pc-widget-head">'+(draft&&!ui.printing?'<button class="pc-icon-button pc-drag-handle" data-drag="'+w.id+'" draggable="'+!saving+'" aria-label="Переместить: '+esc(d.title)+'" '+(saving?'disabled':'')+'>⠿</button>':'')+'<div><h2 tabindex="-1" data-focus="widget-heading-'+w.id+'">'+icon(d.icon)+d.title+'</h2><p>'+d.hint+'</p></div></div><div class="pc-widget-body">'+body+'</div></section>';
    }
    function grid(){
      const visible=current().widgets.filter(w=>w.visible);
      if(ui.expanded&&!ui.printing){const w=current().widgets.find(w=>w.id===ui.expanded);return '<div class="pc-expanded"><button class="pc-button" data-action="back-overview" data-focus="back-overview">← Назад к обзору</button>'+widget({...w,width:'full'})+'</div>';}
      return '<div class="pc-widget-grid" data-columns="'+current().columns+'">'+visible.map(widget).join('')+'</div>'+(!visible.length?empty('Все блоки скрыты. Включите нужные в настройке экрана.'):'');
    }
    function dialog(){
      if(!ui.modal||ui.printing)return '';
      let title='',content='';
      if(ui.modal==='leave'){
        title='Сохранить настройку экрана?';content='<p>В настройке есть неподтверждённые изменения.</p><div class="pc-dialog-actions"><button class="pc-button primary" data-action="leave-save" '+(saving?'disabled':'')+'>Сохранить настройки</button><button class="pc-button" data-action="leave-discard" '+(saving?'disabled':'')+'>Выйти без сохранения</button><button class="pc-button" data-action="leave-continue" '+(saving?'disabled':'')+'>Продолжить настройку</button></div>';
      }else if(ui.modal==='csv'){
        title='Выгрузить этапы по фильтрам';content='<p>'+esc(scopeLabel())+'</p><p><strong>'+number(model.rows.length)+' этапов</strong>. Включены все отобранные этапы независимо от обзорных блоков и открытой таблицы. Выгрузка не является таблицей долей всех соисполнителей.</p><p>Снимок данных: '+dateTime(data.source.generatedAt)+'</p><div class="pc-dialog-actions"><button class="pc-button primary" data-action="confirm-csv">Выгрузить '+number(model.rows.length)+' этапов</button><button class="pc-button" data-action="close-dialog">Отмена</button></div>';
      }else if(ui.modal==='data'){
        title='Источник данных';content='<div class="pc-source-card"><strong>'+esc(sourceLabel())+'</strong><p>'+esc(data.source.label)+'</p><small>Проектов: '+data.projects.length+' · Этапов: '+data.projects.reduce((n,p)=>n+p.stages.length,0)+'</small><small>Время источника: '+dateTime(data.source.generatedAt)+'</small></div><div class="pc-dialog-actions"><button class="pc-button primary" data-action="import-data">Загрузить JSON</button>'+(options.demoFactory?'<button class="pc-button" data-action="demo-data">Учебный набор</button>':'')+'<button class="pc-button" data-action="export-data">Сохранить набор</button></div>';
      }else if(ui.modal==='help'){
        title='Как читать показатели';content='<div class="pc-help"><p><strong>Соисполнители:</strong> этапы общего выбранного заказа по всем его проектам с остальными фильтрами. Доля заказа = назначено / все этапы состава. Выполнено из назначенных = закрыто / назначено исполнителю. Фильтр исполнителя выделяет строку и не меняет знаменатель доли.</p><p><strong>Просрочка:</strong> срок раньше даты расчёта, этап не выполнен. Приостановленные этапы тоже учитываются.</p><p><strong>Ближайшие сроки:</strong> следующие '+current().horizon+' дней; дата расчёта показывается отдельно.</p><p><strong>Выполнение:</strong> расчёт по количеству этапов, без стоимости и трудоёмкости.</p><p><strong>Журнал:</strong> события выбранных проектов; фильтр исполнителя этапа его не ограничивает. Криптографическая проверка подписей остаётся функцией основной программы.</p><p><strong>Дата:</strong> календарные дни, без производственного календаря. Прошлая дата оценивает сроки при текущих статусах, не восстанавливая историю.</p><p><strong>Настройка экрана:</strong> сохраняется отдельно от проектов после подтверждённой записи. Неподтверждённые изменения можно отменить.</p></div>';
      }else if(ui.modal==='detail'){
        const p=data.projects.find(p=>p.projectId===Number(ui.detail.projectId)),s=p&&p.stages.find(s=>s.uid===ui.detail.stageUid);if(!p)return '';
        title=s?s.title:p.name;const rows=s?[s]:p.stages;
        content='<div class="pc-detail-context">'+badge('Заказ № '+p.orderNo)+badge(p.enterprise)+badge('Проект ID '+p.projectId)+'</div><p>'+esc(p.name)+'</p><div class="pc-table-wrap"><table class="pc-table"><thead><tr><th>Этап</th><th>Исполнитель</th><th>Статус</th><th>Начало / срок</th></tr></thead><tbody>'+rows.map(s=>'<tr><td>'+esc(s.title)+'<small>UID '+esc(s.uid)+' · № '+s.seq+'</small><small>Адресаты: '+esc(s.addressees||'—')+'</small><p>'+esc(s.comment||'Комментария нет')+'</p></td><td>'+esc(s.executor)+'</td><td>'+badge(D.labels[s.status])+'</td><td>'+fmt(s.start)+' / '+fmt(s.deadline)+'</td></tr>').join('')+'</tbody></table></div>'+(typeof options.onOpen==='function'?'<div class="pc-dialog-actions"><button class="pc-button primary" data-action="open-host">Открыть в программе</button></div>':'');
      }
      return '<dialog class="pc-dialog '+(ui.modal==='detail'?'wide':'')+'" aria-label="'+esc(title)+'"><div class="pc-dialog-head"><h2>'+esc(title)+'</h2><button class="pc-icon-button" data-action="close-dialog" aria-label="Закрыть" '+(saving?'disabled':'')+'>'+icon('close')+'</button></div><div class="pc-dialog-body">'+content+'</div></dialog>';
    }
    function conflictPanel(){
      if(!ui.conflict)return '';
      const remote=ui.conflict.settings?cleanSettings(ui.conflict.settings):baseSettings();
      const describe=s=>s.widgets.filter(w=>w.visible).map(w=>titleById(w.id).title+' ('+(w.width==='full'?'вся строка':'половина строки')+')').join(' → ')||'Все блоки скрыты';
      return '<div class="pc-conflict pc-notice warn" role="alert"><div><strong>Экран уже сохранён с другого компьютера</strong><p>Сохранённый вариант: '+esc(describe(remote))+'</p><p>'+remote.columns+' колонки · '+(remote.font==='large'?'крупный':'обычный')+' текст · '+remote.horizon+' дней · '+(remote.countMode==='leaves'?'конечные':'все')+' этапы</p><p>Ваш черновик: '+esc(describe(draft||settings))+'</p><p>'+current().columns+' колонки · '+(current().font==='large'?'крупный':'обычный')+' текст · '+current().horizon+' дней · '+(current().countMode==='leaves'?'конечные':'все')+' этапы</p><div class="pc-dialog-actions"><button class="pc-button" data-action="conflict-load" '+(saving?'disabled':'')+'>Загрузить сохранённый вариант</button><button class="pc-button primary" data-action="conflict-own" '+(saving?'disabled':'')+'>Сохранить мой вариант</button></div></div></div>';
    }
    function missingFilters(){
      const missing=[];
      for(const key of ['orderNo','enterprise','executor'])if(filters[key]&&!filteredOptions(key).includes(filters[key]))missing.push(({orderNo:'Заказ',enterprise:'Предприятие',executor:'Исполнитель'})[key]+': '+filters[key]);
      if(filters.projectId&&!data.projects.some(p=>String(p.projectId)===filters.projectId))missing.push('Проект ID '+filters.projectId);
      return missing.length?'<div class="pc-notice warn">Выбранные значения отсутствуют в обновлённом источнике: '+esc(missing.join('; '))+'. Измените отбор или сбросьте фильтры.</div>':'';
    }
    function render(){
      if(destroyed)return;model=D.analyse(data,filters,current());
      const active=document.activeElement,focus=active&&element.contains(active)?active.getAttribute('data-focus'):null,selection=active&&typeof active.selectionStart==='number'?[active.selectionStart,active.selectionEnd]:null;
      const oldBody=element.querySelector('.pc-dialog-body'),modalScroll=oldBody?oldBody.scrollTop:0;
      const theme=current().theme==='system'?(media.matches?'dark':'light'):current().theme;
      element.dataset.theme=theme;element.dataset.font=current().font;element.dataset.editing=draft?'true':'false';
      element.innerHTML='<div class="pc-app"><div class="pc-main"><header class="pc-cycle-appbar"><strong>Производственный цикл</strong><span class="pc-cycle-context">Статистика</span><div class="pc-cycle-top-actions"><button class="pc-button" data-action="data">Данные</button><button class="pc-button" data-action="help">Подсказки</button></div></header><main class="pc-content"><div class="pc-page-head"><div><h1>Статистика</h1><p>Исполнение заказов, сроки и участие соисполнителей</p></div><div class="pc-page-actions"><button class="pc-button" data-action="refresh" '+(busy?'disabled':'')+'>'+icon('refresh')+'Обновить данные</button><button class="pc-button" data-action="export-csv">'+icon('download')+'Выгрузить этапы по фильтрам</button><button class="pc-button" data-action="print">'+icon('print')+'Печать</button>'+(!draft?'<button class="pc-button primary" data-action="settings" data-focus="settings" '+(loading?'disabled':'')+'>'+icon('settings')+'Настроить экран</button>':'')+'</div></div><div class="pc-source-line"><span class="pc-source-pill '+(data.source.kind==='demo'?'demo':'')+'"><i></i>'+esc(sourceLabel())+'</span><span>'+esc(data.source.label)+'</span><span class="pc-last-refresh">Снимок: '+dateTime(data.source.generatedAt)+'</span></div>'+(busy?'<div role="status" class="pc-notice">Обновление данных…</div>':'')+(loading?'<div role="status" class="pc-notice">Загрузка личной настройки…</div>':'')+(loadFailed?'<div role="alert" class="pc-notice error">Личную настройку не удалось загрузить. <button class="pc-button" data-action="retry-settings">Повторить загрузку</button></div>':'')+(data.source.kind==='demo'?'<div class="pc-demo-note">Учебный набор. Названия и показатели приведены для демонстрации.</div>':'')+(ui.message?'<div class="pc-notice '+esc(ui.message.type)+'" role="'+(ui.message.type==='error'?'alert':'status')+'"><span>'+esc(ui.message.message)+'</span><button class="pc-icon-button" data-action="dismiss" aria-label="Скрыть сообщение">'+icon('close')+'</button></div>':'')+(ui.recovered&&!draft?'<div class="pc-notice warn" role="status"><div>Есть неподтверждённый черновик от '+dateTime(ui.recovered.savedAt)+'.<div class="pc-dialog-actions"><button class="pc-button" data-action="recover-draft">Восстановить черновик</button><button class="pc-button" data-action="discard-recovered">Удалить черновик</button></div></div></div>':'')+(ui.legacyCandidate&&!draft?'<div class="pc-notice warn" role="status"><div>Найдена прежняя локальная компоновка. Принадлежность этому профилю и рабочей базе не подтверждена. Просмотрите её перед переносом.<div class="pc-dialog-actions"><button class="pc-button" data-action="import-legacy">Просмотреть и перенести</button><button class="pc-button" data-action="dismiss-legacy">Пропустить</button></div></div></div>':'')+conflictPanel()+'<div class="pc-filters"><label>Заказ<select data-filter="orderNo" data-focus="orderNo">'+selectOptions(filteredOptions('orderNo'),filters.orderNo,'Все заказы')+'</select></label><label>Предприятие<select data-filter="enterprise" data-focus="enterprise">'+selectOptions(filteredOptions('enterprise'),filters.enterprise,'Все предприятия')+'</select></label><label>Исполнитель<select data-filter="executor" data-focus="executor">'+selectOptions(filteredOptions('executor'),filters.executor,'Все исполнители')+'</select></label><label>Дата расчёта сроков<input type="date" data-filter="asOf" data-focus="asOf" value="'+filters.asOf+'"></label><label class="pc-search">Проект или заказ<div>'+icon('search')+'<input data-filter="search" data-focus="search" value="'+esc(filters.search)+'" placeholder="Найти проект…" type="search"></div></label><button class="pc-button subtle pc-filter-reset" data-action="reset-filters">Сбросить</button></div>'+missingFilters()+(filters.projectId?'<div class="pc-filter-note">Выбран проект ID '+esc(filters.projectId)+' <button class="pc-button subtle" data-action="clear-project">Снять фильтр ×</button></div>':'')+'<div class="pc-calculation-note"><strong>Заказ: '+(filters.orderNo?'№ '+esc(filters.orderNo):'все')+' · Проектов: '+model.projects.length+' · Этапов: '+model.rows.length+'</strong><span>'+(current().countMode==='leaves'?'Только конечные этапы':'Все уровни этапов, как в программе')+' · календарные дни · Дата расчёта: '+fmt(model.asOf)+'</span><span>Прошлая дата оценивает сроки при текущих статусах; история выполнения не восстанавливается.</span></div><div class="pc-print-context"><p>'+esc(scopeLabel())+'</p><p>Данные получены: '+dateTime(data.source.generatedAt)+' · Отчёт сформирован: '+dateTime(new Date().toISOString())+'</p></div>'+(!data.projects.length&&!busy?empty('В источнике пока нет проектов.'):data.projects.length&&!model.projects.length?empty('По выбранным фильтрам проектов нет.'):'')+'<div class="pc-kpis">'+[
        ['folder','Незавершённые проекты',model.counts.activeProjects,model.counts.projectOverdue+' с истёкшим сроком','projects'],
        ['flag','Просроченные этапы',model.counts.overdue,'В том числе приостановлено: '+model.counts.heldOverdue,'overdue'],
        ['calendar','Срок в следующие '+current().horizon+' дней',model.counts.dueSoon,'Отдельно на дату расчёта: '+model.counts.today,'soon'],
        ['check','Доля выполненных этапов',model.counts.total?model.counts.donePercent+'%':'—',model.counts.done+' из '+model.counts.total+' этапов','done']
      ].map(([ico,label,value,hint,action])=>'<button class="pc-kpi" data-action="kpi" data-value="'+action+'" data-focus="kpi-'+action+'"><div><span>'+label+'</span>'+icon(ico)+'</div><strong>'+(typeof value==='number'?number(value):esc(value))+'</strong><small>'+esc(hint)+'</small></button>').join('')+'</div><div class="pc-layout-workspace">'+editor()+'<div class="pc-layout-preview" aria-label="'+(draft?'Предпросмотр расположения':'Блоки статистики')+'">'+grid()+'</div></div><footer class="pc-footer"><span>Выполнение оценивается по количеству этапов выбранного состава.</span><span>Источник '+(data.source.consistent?'передан одним набором':'прочитан последовательно')+'</span></footer></main></div></div><input type="file" data-file="data" accept=".json,application/json" hidden>'+dialog();
      const modal=element.querySelector('dialog');if(modal){if(typeof modal.showModal==='function')modal.showModal();else modal.setAttribute('open','');modal.addEventListener('cancel',e=>{e.preventDefault();closeDialog();});modal.querySelector('.pc-dialog-body').scrollTop=modalScroll;}
      if(focus)focusAt(focus,selection);
      requestAnimationFrame(equalizeOverview);
    }
    function focusAt(key,selection){let target=[...element.querySelectorAll('[data-focus]')].find(n=>n.dataset.focus===key);if(target&&target.disabled&&/^move-(up|down)-/.test(key)){const id=key.replace(/^move-(up|down)-/,'');target=[...element.querySelectorAll('[data-focus]')].find(n=>n.dataset.focus==='toggle-'+id);}if(target&&!target.disabled){target.focus({preventScroll:true});if(selection&&typeof target.setSelectionRange==='function'&&['search','text'].includes(target.type))target.setSelectionRange(...selection);}}
    function equalizeOverview(){
      if(destroyed)return;const halves=[...element.querySelectorAll('.pc-widget-grid>.pc-widget.half')];
      for(const item of halves)item.style.minHeight='';
      const height=Math.max(current().font==='large'?440:360,...halves.map(item=>item.getBoundingClientRect().height));
      for(const item of halves)item.style.minHeight=height+'px';
    }
    function clearRecovery(){
      draftGeneration++;if(draftTimer){clearTimeout(draftTimer);draftTimer=null;}
      const generation=draftGeneration;
      draftWrite=draftWrite.catch(()=>{}).then(async()=>{if(adapter&&adapter.clearDraft&&!destroyed&&generation===draftGeneration)await adapter.clearDraft();});
      return draftWrite;
    }
    function queueDraft(){
      if(!draft||saving||destroyed)return;
      if(draftTimer)clearTimeout(draftTimer);const generation=draftGeneration,serial=JSON.stringify(draft),expectedRevision=revision;
      draftTimer=setTimeout(()=>{
        draftTimer=null;draftWrite=draftWrite.catch(()=>{}).then(async()=>{
          if(destroyed||generation!==draftGeneration||!draft)return;
          if(!adapter||!adapter.saveDraft)return;
          try{const result=await adapter.saveDraft(serial,{expectedRevision});if(destroyed||generation!==draftGeneration)return;if(!result||result.state!=='draft_saved')throw Error('Запись черновика не подтверждена.');}
          catch(error){if(!destroyed&&generation===draftGeneration){notify('Копию черновика сохранить не удалось. Можно повторить сохранение или экспортировать черновик. '+error.message,'warn');render();}}
        });
      },300);
    }
    function beginEdit(){if(loading||saving)return;draft=clone(settings);ui.saveFailed=false;draftGeneration++;ui.expanded=null;ui.conflict=null;render();focusAt('editor-heading');}
    async function discardEdit(){if(saving)return false;saving=true;render();try{await clearRecovery();if(destroyed)return false;draft=null;ui.conflict=null;ui.expanded=null;ui.recovered=null;ui.saveFailed=false;ui.message=null;return true;}catch(error){if(!destroyed)notify('Отмена не завершена: копию черновика удалить не удалось. Повторите отмену. '+error.message,'error');return false;}finally{if(!destroyed){saving=false;render();if(!draft)focusAt('settings');}}}
    async function loadSettings(){
      const ticket=++requestId;loading=true;loadFailed=false;render();
      try{
        let loaded=adapter&&adapter.load?await adapter.load():null;if(destroyed||ticket!==requestId)return;
        if(loaded&&loaded.state&&loaded.state!=='loaded')throw Error('Загрузка настройки не подтверждена.');
        const envelope=loaded&&loaded.state==='loaded';let raw=envelope?loaded.settings:loaded;
        if(envelope){revision=loaded.revision||0;location=loaded.location||'local';ui.recovered=loaded.draft||null;if(loaded.warning)notify(loaded.warning,'warn');}
        if(!raw){try{const previous=root.localStorage&&root.localStorage.getItem(legacyKey);if(previous){ui.legacyCandidate=cleanSettings(JSON.parse(previous));}}catch(error){notify('Прежняя локальная настройка недоступна: '+error.message,'warn');}}
        settings=raw?cleanSettings(raw):baseSettings();
        if(raw&&raw.version===1&&adapter&&adapter.save){
          const migrated=await adapter.save(JSON.stringify(settings),{expectedRevision:revision});if(destroyed||ticket!==requestId)return;
          if(migrated&&migrated.state==='saved'){revision=migrated.revision;settings=cleanSettings(migrated.settings||settings);notify('Прежняя компоновка перенесена. Архив доступен в настройке экрана.'+(migrated.warning?' '+migrated.warning:''),migrated.warning?'warn':'ok');}
          else if(migrated&&migrated.state==='conflict'){revision=migrated.revision;settings=migrated.settings?cleanSettings(migrated.settings):baseSettings();notify('Загружена уже сохранённая личная настройка.','warn');}
          else throw Error('Перенос настройки не подтверждён.');
        }
      }catch(error){if(destroyed||ticket!==requestId)return;loadFailed=true;notify('Настройку не удалось загрузить. '+error.message,'error');}
      finally{if(!destroyed&&ticket===requestId){loading=false;render();}}
    }
    async function saveSettings(expected=revision){
      if(!draft||saving||loading||loadFailed)return false;
      if(!adapter||!adapter.save){notify('Хранилище личных настроек не подключено. Экспортируйте черновик или повторите после подключения.','error');render();return false;}
      const serial=JSON.stringify(draft),ticket=requestId;saving=true;ui.saveFailed=false;ui.message=null;render();
      try{
        if(draftTimer){clearTimeout(draftTimer);draftTimer=null;}await draftWrite.catch(()=>{});
        if(adapter.saveDraft){try{const copy=await adapter.saveDraft(serial,{expectedRevision:expected});if(!copy||copy.state!=='draft_saved')throw Error('Запись черновика не подтверждена.');}catch(error){if(!destroyed)notify('Копия черновика не сохранена. '+error.message,'warn');}}
        if(destroyed||ticket!==requestId)return false;
        const result=await adapter.save(serial,{expectedRevision:expected});if(destroyed||ticket!==requestId)return false;
        if(result&&result.state==='conflict'){ui.conflict=result;notify('Ваш черновик сохранён в редакторе. Сравните варианты перед повторной записью.','warn');ui.modal=null;render();return false;}
        if(!result||result.state!=='saved')throw Error('Запись настройки не подтверждена.');
        settings=cleanSettings(result.settings||JSON.parse(serial));revision=result.revision;location=result.location||location;draft=null;ui.conflict=null;ui.recovered=null;
        try{await clearRecovery();}catch(error){if(!destroyed)notify('Настройки сохранены, но прежнюю копию черновика удалить не удалось. '+error.message,'warn');}
        if(destroyed||ticket!==requestId)return false;
        if(result.warning)notify('Настройки сохранены. '+result.warning,'warn');else if(!ui.message)notify('Настройки сохранены.');ui.modal=null;render();focusAt('settings');return true;
      }catch(error){if(!destroyed&&ticket===requestId){ui.saveFailed=true;notify((location==='shared'?'В общей папке не сохранено. ':'Настройки не сохранены. ')+'Черновик остаётся в редакторе. Повторите сохранение. '+error.message,'error');render();}return false;}
      finally{if(!destroyed&&ticket===requestId){saving=false;render();if(draft)queueDraft();}}
    }
    async function saveOwnConflict(){
      if(!ui.conflict||saving)return;const compared=ui.conflict.revision,ticket=requestId;saving=true;render();
      try{const latest=await adapter.load();if(destroyed||ticket!==requestId)return;
        if(!latest||latest.state!=='loaded')throw Error('Не удалось повторно проверить версию.');
        if(latest.revision!==compared){ui.conflict={state:'conflict',revision:latest.revision,settings:latest.settings};notify('Сохранённый вариант снова изменился. Сравните обновлённые варианты.','warn');return;}
        revision=latest.revision;saving=false;await saveSettings(revision);
      }catch(error){if(!destroyed){notify('Версию не удалось проверить: '+error.message,'error');}}
      finally{if(!destroyed&&ticket===requestId){saving=false;render();}}
    }
    function resolveLeave(value){const resolve=leaveResolve;leaveResolve=null;leavePromise=null;ui.modal=null;if(!destroyed)render();if(resolve)resolve(value);}
    function requestLeave(){
      if(destroyed)return Promise.resolve(true);
      if(leavePromise)return leavePromise;
      if(!changed()){if(!draft)return Promise.resolve(true);leavePromise=discardEdit().then(ok=>{leavePromise=null;return ok;});return leavePromise;}
      ui.modal='leave';render();leavePromise=new Promise(resolve=>{leaveResolve=resolve;});return leavePromise;
    }
    function closeDialog(){if(saving)return;if(ui.modal==='leave'){resolveLeave(false);return;}ui.modal=null;render();}
    async function download(content,name,type){
      if(options.onDownload){const path=await options.onDownload(content,name,type);if(destroyed)return;notify(path?'Файл сохранён: '+path:'Файл сохранён.');render();return;}
      const blob=new Blob([content],{type}),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
    }
    const jsonDownload=(value,name)=>download(JSON.stringify(value,null,2),name,'application/json;charset=utf-8');
    function expanded(id){ui.returnView={y:window.scrollY,focus:document.activeElement&&document.activeElement.dataset.focus};ui.expanded=id;render();focusAt('widget-heading-'+id);}
    function backOverview(){const state=ui.returnView,id=ui.expanded;ui.expanded=null;render();requestAnimationFrame(()=>{if(state){window.scrollTo(0,state.y);focusAt(state.focus||'show-all-'+id);}});}
    function resetFilters(){filters={orderNo:'',enterprise:'',executor:'',projectId:'',search:'',asOf:D.today()};ui.calendarDay='';}
    function applyData(raw){if(destroyed)return;data=D.normalize(raw);notify('Данные загружены: '+data.projects.length+' проектов.');render();}
    async function refresh(){
      if(busy||destroyed)return;const ticket=requestId;busy=true;render();
      try{let raw;if(options.provider)raw=await options.provider();else if(data.source.kind==='demo'&&options.demoFactory)raw=options.demoFactory();else{notify('Для обновления этого снимка загрузите новый JSON.');return;}if(!destroyed&&ticket===requestId)applyData(raw);}
      catch(error){if(!destroyed&&ticket===requestId)notify('Обновление не выполнено. Показан предыдущий набор от '+dateTime(data.source.generatedAt)+'. Повторите обновление. '+error.message,'error');}
      finally{if(!destroyed&&ticket===requestId){busy=false;render();}}
    }
    function move(id,delta,target){
      if(!draft||saving)return;const from=draft.widgets.findIndex(w=>w.id===id),to=target?draft.widgets.findIndex(w=>w.id===target):from+delta;
      if(from<0||to<0||to>=draft.widgets.length||from===to)return;
      const [item]=draft.widgets.splice(from,1);draft.widgets.splice(to,0,item);queueDraft();render();
    }
    async function click(e){
      const button=e.target.closest('[data-action]');if(!button||!element.contains(button)||button.disabled)return;
      const action=button.dataset.action;
      try{
        if(action==='settings'){beginEdit();return;}
        if(action==='import-legacy'){if(saving||draft)return;draft=clone(ui.legacyCandidate);ui.legacyCandidate=null;draftGeneration++;queueDraft();notify('Прежняя компоновка показана в предпросмотре. Для переноса подтвердите сохранение.','warn');render();return;}
        if(action==='dismiss-legacy'){ui.legacyCandidate=null;render();return;}
        if(action==='save-settings'){const ok=await saveSettings();if(ok&&leaveResolve)resolveLeave(true);return;}
        if(action==='cancel-settings'){await discardEdit();return;}
        if(action==='reset-settings'){if(draft&&!saving){const archive=draft.legacyArchive;draft=baseSettings();draft.legacyArchive=archive;queueDraft();render();}return;}
        if(action==='retry-settings'){await loadSettings();return;}
        if(action==='recover-draft'){if(saving||draft)return;draft=cleanSettings(ui.recovered.settings);const expected=ui.recovered.expectedRevision;ui.recovered=null;draftGeneration++;if(expected!==revision)ui.conflict={state:'conflict',revision,settings};notify('Черновик восстановлен. Проверьте и сохраните настройки.','warn');render();return;}
        if(action==='discard-recovered'){if(saving||draft)return;saving=true;render();try{await clearRecovery();ui.recovered=null;}finally{saving=false;render();}return;}
        if(action==='conflict-load'){await clearRecovery();settings=ui.conflict.settings?cleanSettings(ui.conflict.settings):baseSettings();revision=ui.conflict.revision;draft=null;ui.conflict=null;if(leaveResolve)resolveLeave(true);else{notify('Загружен сохранённый вариант.');render();}return;}
        if(action==='conflict-own'){await saveOwnConflict();if(!draft&&leaveResolve)resolveLeave(true);return;}
        if(action==='leave-save'){if(await saveSettings())resolveLeave(true);return;}
        if(action==='leave-discard'){if(await discardEdit())resolveLeave(true);return;}
        if(action==='leave-continue'){resolveLeave(false);return;}
        if(action==='close-dialog'){closeDialog();return;}
        if(action==='dismiss'){ui.message=null;render();return;}
        if(action==='data'||action==='help'){ui.modal=action;render();return;}
        if(action==='reset-filters'||action==='all-projects'){resetFilters();render();return;}
        if(action==='clear-project'){filters.projectId='';render();return;}
        if(action==='refresh'){await refresh();return;}
        if(action==='filter-group'){filters[button.dataset.type]=button.dataset.value;render();return;}
        if(action==='risk-mode'){ui.tableMode=button.dataset.value;ui.calendarDay='';render();return;}
        if(action==='calendar-day'){ui.calendarDay=button.dataset.date;expanded('risks');return;}
        if(action==='clear-day'){ui.calendarDay='';render();return;}
        if(action==='show-all'){expanded(button.dataset.widget);return;}
        if(action==='back-overview'){backOverview();return;}
        if(action==='detail'){ui.detail={projectId:Number(button.dataset.project),stageUid:button.dataset.stage||null};ui.modal='detail';render();return;}
        if(action==='open-host'){if(await requestLeave()){await options.onOpen({...ui.detail});if(!destroyed){ui.modal=null;render();}}return;}
        if(action==='kpi'){const value=button.dataset.value;if(value==='projects')expanded('portfolio');else if(value==='done')expanded('statuses');else{ui.tableMode=value;ui.calendarDay='';expanded('risks');}return;}
        if(action==='move-up'||action==='move-down'){const focus=button.dataset.focus;move(button.dataset.widget,action==='move-up'?-1:1);focusAt(focus);return;}
        if(action==='export-settings'){await jsonDownload({type:'production-cycle-dashboard-settings',...current()},'statistics-settings.json');return;}
        if(action==='export-archive'){await jsonDownload({type:'production-cycle-dashboard-settings-archive',version:1,archive:current().legacyArchive},'statistics-legacy-settings.json');return;}
        if(action==='import-data'){element.querySelector('[data-file="data"]').click();return;}
        if(action==='export-data'){await jsonDownload(data,'statistics-data.json');return;}
        if(action==='demo-data'){applyData(options.demoFactory());ui.modal=null;render();return;}
        if(action==='export-csv'){ui.modal='csv';render();return;}
        if(action==='confirm-csv'){const csv=D.toCSV(model.rows),name='statistics-stages-'+model.asOf+'.csv';ui.modal=null;await download(csv,name,'text/csv;charset=utf-8');render();return;}
        if(action==='print'){ui.modal=null;beforePrint();try{if(options.onPrint)await options.onPrint();else window.print();}finally{afterPrint();}return;}
      }catch(error){if(!destroyed){notify(error.message,'error');render();}}
    }
    async function change(e){
      const target=e.target;
      try{
        if(target.dataset.filter){const key=target.dataset.filter;if(key==='asOf'&&!D.validDate(target.value))throw Error('Выберите корректную дату.');filters[key]=target.value;ui.calendarDay='';render();return;}
        if(draft&&!saving){
          if(target.dataset.setting){const key=target.dataset.setting;draft[key]=['horizon','columns'].includes(key)?Number(target.value):target.value;queueDraft();render();return;}
          if(target.dataset.widgetToggle){draft.widgets.find(w=>w.id===target.dataset.widgetToggle).visible=target.checked;queueDraft();render();return;}
          if(target.dataset.widgetWidth){draft.widgets.find(w=>w.id===target.dataset.widgetWidth).width=target.value;queueDraft();render();return;}
        }
        if(target.dataset.control==='barMetric'){ui.barMetric=target.value;render();return;}
        if(target.dataset.file){const file=target.files[0];if(!file)return;if(file.size>10*1024*1024)throw Error('Размер JSON превышает 10 МиБ.');const raw=JSON.parse(await file.text());if(destroyed)return;applyData({...raw,source:{...raw.source,kind:'import',label:'Импорт: '+file.name}});ui.modal=null;render();}
      }catch(error){if(!destroyed){notify(error.message,'error');render();}}
    }
    function input(e){if(e.target.dataset.filter==='search'){filters.search=e.target.value;ui.calendarDay='';render();}}
    async function escapeKey(e){
      if(e.key!=='Escape')return;
      if(ui.dragId){e.preventDefault();e.stopPropagation();ui.dragId=null;ui.dragCancelled=true;clearDropTargets();return;}
      if(ui.modal){e.preventDefault();closeDialog();return;}
      if(draft&&!saving){e.preventDefault();await discardEdit();}
    }
    function clearDropTargets(){ui.dragTarget=null;for(const n of element.querySelectorAll('.pc-drop-target'))n.classList.remove('pc-drop-target');}
    function dragstart(e){const handle=e.target.closest('[data-drag]');if(!draft||saving||!handle){e.preventDefault();return;}ui.dragId=handle.dataset.drag;ui.dragCancelled=false;e.dataTransfer.effectAllowed='move';e.dataTransfer.setData('text/plain',ui.dragId);}
    function dragover(e){const item=e.target.closest('[data-drop-widget]');if(!draft||!ui.dragId||!item||ui.dragId===item.dataset.dropWidget)return;e.preventDefault();clearDropTargets();ui.dragTarget=item.dataset.dropWidget;item.classList.add('pc-drop-target');e.dataTransfer.dropEffect='move';}
    function drop(e){const item=e.target.closest('[data-drop-widget]'),id=ui.dragId;clearDropTargets();ui.dragId=null;if(!item||!id||ui.dragCancelled)return;e.preventDefault();move(id,0,item.dataset.dropWidget);focusAt('widget-heading-'+id);}
    function dragend(){ui.dragId=null;clearDropTargets();}
    const beforePrint=()=>{if(!destroyed){ui.printing=true;ui.modal=null;render();}},afterPrint=()=>{if(!destroyed){ui.printing=false;render();}};
    const beforeUnload=e=>{if(changed()){e.preventDefault();e.returnValue='';}};
    const media=matchMedia('(prefers-color-scheme: dark)'),mediaChange=()=>{if(current().theme==='system')render();};
    element.dataset.pcd='';element.dataset.embedded=options.embedded?'true':'false';
    const handlers={click,change,input,keydown:escapeKey,dragstart,dragover,drop,dragend};for(const [name,handler] of Object.entries(handlers))element.addEventListener(name,handler);
    window.addEventListener('beforeprint',beforePrint);window.addEventListener('afterprint',afterPrint);window.addEventListener('beforeunload',beforeUnload);window.addEventListener('resize',equalizeOverview);media.addEventListener('change',mediaChange);
    render();const ready=loadSettings();
    const api={ready,refresh,setData:applyData,requestLeave,isEditing:()=>!!draft,hasUnsavedChanges:changed,getSettings:()=>clone(settings),getDraft:()=>draft?clone(draft):null,getFilters:()=>clone(filters),getData:()=>clone(data),getMetrics:()=>clone(model.counts),
      setSettings:async raw=>{await ready;if(destroyed)return false;draft=cleanSettings(raw);return saveSettings();},
      setFilters:next=>{for(const key of ['orderNo','enterprise','executor','projectId','search','asOf'])if(next[key]!==undefined){if(key==='asOf'&&!D.validDate(next[key]))throw Error('Некорректная дата.');filters[key]=String(next[key]);}ui.calendarDay='';render();},
      destroy:()=>{destroyed=true;requestId++;draftGeneration++;if(draftTimer)clearTimeout(draftTimer);if(leaveResolve){leaveResolve(false);leaveResolve=null;}for(const [name,handler] of Object.entries(handlers))element.removeEventListener(name,handler);window.removeEventListener('beforeprint',beforePrint);window.removeEventListener('afterprint',afterPrint);window.removeEventListener('beforeunload',beforeUnload);window.removeEventListener('resize',equalizeOverview);media.removeEventListener('change',mediaChange);element.innerHTML='';for(const key of ['pcd','theme','font','embedded','editing'])delete element.dataset[key];}
    };
    if(options.provider&&!options.data)void ready.then(refresh);return api;
  }
  root.ProductionDashboards=Object.freeze({mount,cleanSettings,version:'2.0.0'});
})(globalThis);
