'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const D=require('../frontend/dashboards/js/domain.js');
const stage=(uid,status,deadline,parentUid=null)=>({uid,seq:1,parentUid,title:uid,executor:'Иванов',start:'2026-10-01',deadline,status,addressees:'Другой завод'});
const fixture=()=>({schemaVersion:1,source:{asOf:'2026-11-14'},projects:[{projectId:1,orderNo:'Один заказ',name:'Проект 1',enterprise:'КГЭ',start:'2026-10-01',deadline:'2026-11-13',stages:[stage('a','hold','2026-11-12'),stage('b','done','2026-11-01'),stage('c','work','2026-11-14'),stage('d','new','2026-11-21')]},{projectId:2,orderNo:'Один заказ',name:'Проект 2',enterprise:'Молот',start:'2026-10-01',deadline:'2026-11-22',stages:[stage('a','work','2026-11-15')]}]});
test('date arithmetic uses calendar days over leap days and DST',()=>{assert.equal(D.validDate('2026-02-29'),false);assert.equal(D.validDate('2024-02-29'),true);assert.equal(D.days('2026-03-30','2026-03-29'),1);assert.equal(D.addDays('2024-02-28',1),'2024-02-29');});
test('held overdue included; done excluded; today separate from next seven days',()=>{const m=D.analyse(D.normalize(fixture()));assert.equal(m.counts.overdue,1);assert.equal(m.counts.heldOverdue,1);assert.equal(m.counts.today,1);assert.equal(m.counts.dueSoon,2);assert.equal(m.counts.donePercent,20);assert.equal(m.counts.projectOverdue,1);});
test('same order and same stage UID in different projects remain independent',()=>{const m=D.analyse(D.normalize(fixture()));assert.equal(m.projects.length,2);assert.equal(new Set(m.rows.map(s=>s.key)).size,5);assert.equal(m.enterprises.find(e=>e.name==='КГЭ').total,4);});
test('enterprise uses project field and executor filters stage population',()=>{const f=fixture();f.projects[1].stages[0].executor='Петров';const m=D.analyse(D.normalize(f),{executor:'Петров'});assert.equal(m.rows.length,1);assert.equal(m.enterprises[0].name,'Молот');});
test('leaf mode excludes summary stage from the denominator',()=>{const f=fixture();f.projects[0].stages[1].parentUid='a';const d=D.normalize(f);assert.equal(D.analyse(d,{}, {countMode:'leaves'}).rows.length,4);});
test('cycle and missing parent rejected before replacing dataset',()=>{let f=fixture();f.projects[0].stages[0].parentUid='a';assert.throws(()=>D.normalize(f),/Цикл/);f=fixture();f.projects[0].stages[0].parentUid='missing';assert.throws(()=>D.normalize(f),/Не найден/);});
test('unknown statuses, duplicate projects and duplicate local UIDs rejected',()=>{let f=fixture();f.projects[0].stages[0].status='overdue';assert.throws(()=>D.normalize(f),/статус/);f=fixture();f.projects[1].projectId=1;assert.throws(()=>D.normalize(f),/projectId/);f=fixture();f.projects[0].stages[1].uid='a';assert.throws(()=>D.normalize(f),/Повтор/);});
test('raw native snapshot supported without guessing unknown fields',()=>{const p=fixture().projects[0];const d=D.normalize({databaseId:91,project:{...p},stages:p.stages});assert.equal(d.projects[0].projectId,91);assert.throws(()=>D.normalize({project:{...p},stages:p.stages}),/projectId/);});
test('no stages produces zero counters and unfinished project',()=>{const f=fixture();f.projects=[{...f.projects[0],stages:[]}];const m=D.analyse(D.normalize(f));assert.equal(m.counts.total,0);assert.equal(m.counts.donePercent,0);assert.equal(m.counts.activeProjects,1);assert.equal(m.quality.emptyProjects,1);});
test('CSV escapes formulas, quotes and Unicode for Excel',()=>{const m=D.analyse(D.normalize(fixture()));m.rows[0].title='=HYPERLINK("bad")';const csv=D.toCSV(m.rows);assert.ok(csv.startsWith('\uFEFF'));assert.ok(csv.includes('"\'=HYPERLINK(""bad"")"'));});
test('Tauri provider reads only known commands and rejects a changed source',async()=>{const ctx={PCDomain:D};vm.createContext(ctx);vm.runInContext(fs.readFileSync(path.join(__dirname,'../frontend/dashboards/js/tauri-adapter.js'),'utf8'),ctx);let listCalls=0;const calls=[];const provider=ctx.createTauriDashboardProvider(async(cmd,args)=>{calls.push(cmd);if(cmd==='production_list_projects'){listCalls++;return [{projectId:1,updatedAt:listCalls===1?'old':'new',stageCount:1,doneCount:0}];}if(cmd==='production_load_snapshot_by_id')return {databaseId:1,project:fixture().projects[0],stages:[]};throw Error('Unexpected mutation');});await assert.rejects(provider(),/менялись/);assert.deepEqual(calls,['production_list_projects','production_load_snapshot_by_id','production_list_projects']);});
test('coexecutor shares combine all projects of one order and separate own completion',()=>{const f=fixture();f.projects[0].stages[2].executor='Петров';f.projects[1].stages[0].executor='Петров';const p=D.participation(D.normalize(f),{}, {},'Один заказ');assert.equal(p.projectCount,2);assert.equal(p.total,5);const i=p.members.find(m=>m.executor==='Иванов'),other=p.members.find(m=>m.executor==='Петров');assert.equal(i.assigned,3);assert.equal(i.sharePercent,60);assert.equal(i.donePercent,33.3);assert.equal(other.sharePercent,40);assert.equal(other.donePercent,0);});
test('performer filter highlights member without changing participation denominator',()=>{const f=fixture();f.projects[1].stages[0].executor='Петров';const d=D.normalize(f),p=D.participation(d,{executor:'Петров',orderNo:'Один заказ'});assert.equal(p.total,5);assert.equal(p.members.length,2);assert.equal(p.members.find(m=>m.executor==='Петров').sharePercent,20);assert.equal(p.members.find(m=>m.executor==='Петров').highlighted,true);});
test('participation denominator respects enterprise scope and leaf calculation mode',()=>{const f=fixture();f.projects[0].stages[1].parentUid='a';const p=D.participation(D.normalize(f),{enterprise:'КГЭ',orderNo:'Один заказ'}, {countMode:'leaves'});assert.equal(p.projectCount,1);assert.equal(p.total,3);assert.equal(p.members.reduce((n,m)=>n+m.assigned,0),3);});
test('empty order participation gives finite zero metrics',()=>{const f=fixture();f.projects[0].stages=[];f.projects=[f.projects[0]];const p=D.participation(D.normalize(f),{orderNo:'Один заказ'});assert.equal(p.total,0);assert.equal(p.donePercent,0);assert.equal(p.members.length,0);assert.equal(p.orderNo,'Один заказ');});

test('All orders never substitutes an arbitrary first order for participation',()=>{
 const f=fixture();f.projects[1].orderNo='Второй заказ';const d=D.normalize(f);
 const p=D.participation(d);
 assert.deepEqual(p.orders,['Второй заказ','Один заказ']);assert.equal(p.orderNo,'');assert.equal(p.projectCount,0);assert.equal(p.total,0);assert.deepEqual(p.members,[]);
 assert.equal(D.participation(d,{orderNo:'Исчезнувший заказ'}).orderNo,'Исчезнувший заказ');
 assert.equal(D.participation(d,{orderNo:'Исчезнувший заказ'}).members.length,0);
});
test('top order filter includes every project under that order before other filters',()=>{
 const f=fixture();f.projects.push({...f.projects[1],projectId:3,orderNo:'Другой заказ',stages:[stage('third','done','2026-11-13')]});
 const d=D.normalize(f),all=D.analyse(d,{orderNo:'Один заказ'}),enterprise=D.analyse(d,{orderNo:'Один заказ',enterprise:'КГЭ'});
 assert.equal(all.projects.length,2);assert.equal(all.rows.length,5);assert.ok(all.rows.every(s=>s.orderNo==='Один заказ'));
 assert.equal(enterprise.projects.length,1);assert.equal(enterprise.rows.length,4);
 assert.equal(D.analyse(d,{orderNo:'Нет заказа'}).rows.length,0);
});
test('one completed assignment out of five gives share 20 percent and own completion 100 percent',()=>{
 const f=fixture();f.projects[0].stages.forEach(s=>s.executor='Основной');f.projects[1].stages[0].executor='Один этап';f.projects[1].stages[0].status='done';
 const d=D.normalize(f),filters={orderNo:'Один заказ',executor:'Один этап'},p=D.participation(d,filters),m=p.members.find(s=>s.executor==='Один этап');
 assert.equal(p.total,5);assert.equal(m.assigned,1);assert.equal(m.done,1);assert.equal(m.sharePercent,1/5*100);assert.equal(m.donePercent,1/1*100);assert.equal(m.highlighted,true);
 assert.equal(p.members.length,2);assert.equal(D.analyse(d,filters).rows.length,1);
});
test('stage CSV contains the full filtered population beyond preview and table pages',()=>{
 const f=fixture();f.projects=[{...f.projects[0],stages:Array.from({length:100},(_,i)=>({...stage('row-'+i,i%2?'work':'done','2026-11-20'),executor:i%4?'Большинство':'Отбор',seq:i+1}))}];
 const d=D.normalize(f),m=D.analyse(d,{orderNo:'Один заказ',executor:'Отбор'}),csv=D.toCSV(m.rows);
 assert.equal(m.rows.length,25);assert.equal(csv.split('\r\n').length,26);assert.ok(csv.includes('row-96'));assert.ok(!csv.includes('row-99'));assert.equal(csv.split('\uFEFF').length-1,1);
});
test('executor filter does not silently narrow the project audit log',()=>{
 const f=fixture();f.projects[1].stages[0].executor='Петров';f.source.auditAvailable=true;
 f.events=f.projects.map((p,i)=>({eventId:String(i),projectId:p.projectId,eventType:'project_changed',actorName:'Автор',createdAt:'2026-11-14T12:00:00Z',comment:'Событие проекта '+p.projectId,changes:[]}));
 const m=D.analyse(D.normalize(f),{orderNo:'Один заказ',executor:'Петров'});
 assert.equal(m.rows.length,1);assert.equal(m.events.length,2);assert.deepEqual(m.events.map(e=>e.projectId).sort(),[1,2]);
});
function dashboardSettings(){const context={PCDomain:D};vm.createContext(context);vm.runInContext(fs.readFileSync(path.join(__dirname,'../frontend/dashboards/js/dashboard.js'),'utf8'),context);return raw=>JSON.parse(JSON.stringify(context.ProductionDashboards.cleanSettings(raw)));}
test('single-layout defaults expose six approved widgets without live presets or named views',()=>{
 const settings=dashboardSettings()({version:2});
 assert.equal(settings.version,2);assert.equal(settings.columns,2);assert.deepEqual(settings.widgets.filter(w=>w.visible).map(w=>w.id),['participation','enterprises','portfolio','calendar','risks','statuses']);
 assert.equal(Object.hasOwn(settings,'preset'),false);assert.equal(Object.hasOwn(settings,'savedViews'),false);assert.equal(settings.legacyArchive,null);
});
test('legacy migration keeps order visibility calculation and full widths with an exact archive',()=>{
 const legacy={version:1,preset:'custom',theme:'dark',font:'large',horizon:30,countMode:'leaves',widgets:[{id:'quality',visible:false,width:'half'},{id:'participation',visible:true,width:'full'},{id:'portfolio',visible:true,width:'full'}],savedViews:[{name:'Старый вариант',settings:{version:1,preset:'manager',widgets:[]}}]};
 const clean=dashboardSettings(),result=clean(legacy);
 assert.deepEqual(result.widgets.slice(0,3),legacy.widgets);assert.equal(result.countMode,'leaves');assert.equal(result.horizon,30);assert.equal(result.font,'large');assert.equal(result.theme,'dark');assert.deepEqual(result.legacyArchive,legacy);
 assert.deepEqual(clean(result),result,'A second load must not duplicate or replace the original migration archive');
 assert.deepEqual(legacy.savedViews[0].name,'Старый вариант','Cleaning must not mutate the source document');
});
test('v2 layout validation removes duplicate and unknown widgets while retaining full width',()=>{
 const clean=dashboardSettings(),result=clean({version:2,columns:1,widgets:[{id:'participation',visible:true,width:'full'},{id:'participation',visible:false,width:'half'},{id:'unknown',visible:true,width:'full'}]});
 assert.equal(result.columns,1);assert.equal(new Set(result.widgets.map(w=>w.id)).size,9);assert.equal(result.widgets[0].width,'full');assert.equal(result.widgets[0].visible,true);assert.equal(result.widgets.some(w=>w.id==='unknown'),false);
 assert.throws(()=>clean({version:3}),/Неподдерживаемый/);
});
