(() => {
 'use strict';
 const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
 const role=value=>({admin:'Администратор',reviewer:'Проверяющий',project_manager:'Руководитель проекта'}[value]||value);
 const event=value=>({project_created:'Создан проект',project_changed:'Изменён проект',project_saved:'Сохранён проект',stage_completed:'Подтверждено выполнение',dictionary_replaced:'Изменён справочник'}[value]||value);
 const date=value=>{if(!value)return 'Пока нет записей';const m=String(value).match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$/);return m?m[3]+'.'+m[2]+'.'+m[1]+' '+m[4]+' '+(m[5]==='Z'?'UTC':m[5]):String(value);};
 const stateOf=user=>({displayName:user.displayName,role:user.role,active:user.active,keyFingerprint:user.keyFingerprint});
 function create(options){
  const mount=options.container;let target=null,mode=null,request=0,offset=0,busy=false,result=null;
  const field=name=>mount.querySelector('[data-user-field="'+name+'"]');
  const action=name=>mount.querySelector('[data-user-action="'+name+'"]');
  const current=()=>options.getAdmin();
  function close(){request++;target=null;mode=null;result=null;offset=0;busy=false;mount.replaceChildren();mount.hidden=true;}
  function message(text,kind='err'){const node=mount.querySelector('[data-user-message]');if(node){node.className='notice '+kind;node.textContent=text;node.setAttribute('role',kind==='err'?'alert':'status');}}
  function buttons(disabled){busy=disabled;for(const node of mount.querySelectorAll('button,input,select,textarea'))node.disabled=(disabled&&!(mode==='statistics'&&node.dataset.userAction==='close'))||node.dataset.locked==='true';}
  function shell(user,nextMode){
   close();if(!current())return;target={...user};mode=nextMode;mount.hidden=false;mount.className='user-admin-panel';
   mount.innerHTML='<div class="user-admin-head"><h3 tabindex="-1">'+(mode==='edit'?'Изменить пользователя: ':'Статистика пользователя: ')+esc(user.displayName)+'</h3><button type="button" class="btn small" data-user-action="close">Закрыть</button></div><div data-user-message aria-live="polite"></div>';
  }
  function edit(user){
   shell(user,'edit');if(!target)return;
   const admin=user.role==='admin';
   mount.insertAdjacentHTML('beforeend','<p class="user-admin-note">Изменяются только ФИО, роль и разрешение входа. Персональный PIN и ключ подписи сохраняются. PIN пользователя здесь недоступен.</p><form data-user-form="edit"><div class="user-admin-form"><label>ФИО<input data-user-field="name" maxlength="160" required value="'+esc(user.displayName)+'"></label><label>Роль<select data-user-field="role" '+(admin?'disabled data-locked="true"':'')+'>'+(admin?'<option value="admin">Администратор</option>':'<option value="project_manager" '+(user.role==='project_manager'?'selected':'')+'>Руководитель проекта</option><option value="reviewer" '+(user.role==='reviewer'?'selected':'')+'>Проверяющий</option>')+'</select></label><label class="user-admin-check user-admin-full"><input type="checkbox" data-user-field="active" '+(user.active?'checked':'')+' '+(admin?'disabled data-locked="true"':'')+'>Разрешить вход и подпись</label><label class="user-admin-full">Причина изменения<textarea data-user-field="comment" maxlength="2000" required></textarea></label><label>PIN администратора<input data-user-field="admin-pin" type="password" autocomplete="off" required></label></div><p class="user-admin-note">'+(admin?'Единственного администратора нельзя заблокировать или перевести в другую роль.':'Отключение запрещает новый вход и подтверждение изменений. Уже открытые данные не удаляются с другого компьютера.')+'</p><div class="user-admin-actions"><button class="btn primary" type="submit" data-user-action="save">Сохранить изменения</button><button class="btn" type="button" data-user-action="close">Отмена</button></div></form>');
   mount.querySelector('h3').focus();
  }
  function statistics(user){
   shell(user,'statistics');if(!target)return;
   mount.insertAdjacentHTML('beforeend','<p class="user-admin-note">Данные производственного журнала по постоянному ID пользователя. Входы в программу и создание профилей в эти показатели не включаются. Это записи в программе, а не часы работы и не оценка производительности. Период включает обе даты; используется дата записи журнала.</p><form data-user-form="statistics"><div class="user-admin-form"><label>С даты<input type="date" data-user-field="from"></label><label>По дату<input type="date" data-user-field="to"></label><label>PIN администратора<input type="password" data-user-field="admin-pin" autocomplete="off" required></label></div><div class="user-admin-actions"><button class="btn primary" type="submit" data-user-action="load">Показать статистику</button><button class="btn" type="button" data-user-action="all-time">Весь период</button></div></form><div data-user-statistics-result></div>');
   mount.querySelector('h3').focus();
  }
  function renderStatistics(value){
   result=value;const s=value.summary,count=s.journalEntries,first=count?value.offset+1:0,last=value.offset+value.events.length;
   const metrics=[['Записей производственного журнала',s.journalEntries],['Сохранений проектов',s.projectSaves],['Подтверждений выполнения',s.completionConfirmations],['Проектов с действиями',s.projectsTouched]];
   mount.querySelector('[data-user-statistics-result]').innerHTML='<div class="user-admin-metrics">'+metrics.map(([label,n])=>'<div class="user-admin-metric"><small>'+label+'</small><strong>'+Number(n)+'</strong></div>').join('')+'</div><p>Последняя запись в выбранном периоде: <b>'+esc(date(s.lastRecordedAt))+'</b></p><p class="user-admin-note">Роль сейчас: '+esc(role(value.user.role))+' · '+(value.user.active?'вход разрешён':'вход и подпись заблокированы')+'. Сохранение проекта и подтверждение этапа — разные записи; их количество не равно числу назначенных пользователю работ.</p>'+(value.userChangeLogValid?'':'<div role="alert" class="notice err">Журнал управления профилями не прошёл проверку целостности. Откройте «Проверка целостности».</div>')+'<div class="user-admin-table-wrap"><table class="user-admin-table"><thead><tr><th>Дата</th><th>Событие</th><th>Заказ и проект</th><th>Комментарий</th></tr></thead><tbody>'+value.events.map(e=>'<tr><td>'+esc(date(e.createdAt))+'</td><td>'+esc(event(e.eventType))+(e.stageRegNumber?'<br>Этап '+esc(e.stageRegNumber):'')+'</td><td>№ '+esc(e.orderNo)+'<br>'+esc(e.projectName)+'</td><td>'+esc(e.comment)+'<br><small>Подписал: '+esc(e.actorName)+'</small></td></tr>').join('')+'</tbody></table></div>'+(count?'':'<p>За выбранный период записей нет.</p>')+'<div class="user-admin-pagination"><span>Показано '+first+'–'+last+' из '+count+'</span><button class="btn small" data-user-action="previous" '+(value.offset?'':'disabled')+'>Предыдущие</button><button class="btn small" data-user-action="next" '+(value.hasMore?'':'disabled')+'>Следующие</button></div><h4>Последние изменения этого профиля (до 20 записей)</h4><div class="user-admin-history">'+(value.profileChanges.length?value.profileChanges.map(e=>'<article><b>'+esc(date(e.createdAt))+' · '+esc(e.actorName)+'</b><div>ФИО: '+esc(e.before.displayName)+' → '+esc(e.after.displayName)+'</div><div>Роль: '+esc(role(e.before.role))+' → '+esc(role(e.after.role))+'</div><div>Вход и подпись: '+(e.before.active?'разрешены':'заблокированы')+' → '+(e.after.active?'разрешены':'заблокированы')+'</div><p>'+esc(e.comment)+'</p></article>').join(''):'<p>'+(value.userChangeLogValid?'Подписанных изменений профиля пока нет.':'История профиля недоступна до проверки целостности.')+'</p>')+'</div><p class="user-admin-note">Проверка целостности производственного журнала доступна в отдельной вкладке администратора. События пользователя сохраняются после смены ФИО или роли.</p>';
  }
  async function load(nextOffset=0){
   const admin=current();if(busy||!target||mode!=='statistics'||!admin)return;
   const pin=field('admin-pin').value,from=field('from').value||null,to=field('to').value||null;
   if(!pin){message('Введите PIN администратора.');field('admin-pin').focus();return;}
   if(from&&to&&from>to){message('Начало периода не может быть позже окончания.');return;}
   const ticket=++request,epoch=options.getEpoch(),id=target.id;result=null;mount.querySelector('[data-user-statistics-result]').replaceChildren();buttons(true);message('Загрузка…','info');
   try{const value=await options.backend.auditUserStatistics(id,admin.id,pin,from,to,nextOffset);if(ticket!==request||epoch!==options.getEpoch()||current()?.id!==admin.id)return;offset=value.offset;renderStatistics(value);message('Данные загружены.','ok');}
   catch(error){if(ticket===request&&epoch===options.getEpoch())message(String(error));}
   finally{if(ticket===request){buttons(false);if(result){action('previous').disabled=result.offset===0;action('next').disabled=!result.hasMore;}}}
  }
  async function save(){
   const admin=current();if(busy||!target||mode!=='edit'||!admin)return;
   const displayName=field('name').value.trim(),comment=field('comment').value.trim(),pin=field('admin-pin').value;
   if(!displayName||!comment||!pin){message('Заполните ФИО, причину изменения и PIN администратора.');return;}
   const ticket=++request,epoch=options.getEpoch();buttons(true);message('Сохранение…','info');
   try{
    const response=await options.backend.updateAuditUser({targetUserId:target.id,displayName,role:field('role').value,active:field('active').checked,expected:stateOf(target),adminUserId:admin.id,adminPin:pin,comment});
    if(ticket!==request||epoch!==options.getEpoch()||current()?.id!==admin.id)return;
    field('admin-pin').value='';await options.onSaved(response.user);if(ticket!==request||epoch!==options.getEpoch())return;
    target={...response.user};mount.querySelector('h3').textContent='Изменить пользователя: '+target.displayName;field('name').value=target.displayName;field('role').value=target.role;field('active').checked=target.active;field('comment').value='';
    message(response.backupWarning||'Профиль изменён. Событие подписано администратором; PIN, ключ и личный экран сохранены.',response.backupWarning?'err':'ok');
   }catch(error){if(ticket===request&&epoch===options.getEpoch())message(String(error));}
   finally{if(ticket===request){field('admin-pin').value='';buttons(false);}}
  }
  mount.addEventListener('submit',e=>{e.preventDefault();if(mode==='edit')void save();else void load(0);});
  mount.addEventListener('input',e=>{if(mode==='statistics'&&!busy&&['from','to'].includes(e.target.dataset.userField)){result=null;offset=0;mount.querySelector('[data-user-statistics-result]').replaceChildren();message('Период изменён. Нажмите «Показать статистику».','info');}});
  mount.addEventListener('click',e=>{const button=e.target.closest('[data-user-action]');if(!button||button.disabled)return;const name=button.dataset.userAction;if(name==='close')close();if(name==='all-time'){field('from').value='';field('to').value='';void load(0);}if(name==='previous')void load(Math.max(0,offset-50));if(name==='next')void load(offset+50);});
  return Object.freeze({edit,statistics,close});
 }
 window.ProductionUserAdministration=Object.freeze({create});
})();
