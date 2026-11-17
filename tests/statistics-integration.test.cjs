'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {create}=require('../frontend/dashboards/integration.js');
function setup(overrides={},panelOptions={}){
 const container={children:[],replaceChildren(){this.children=[];},append(node){this.children.push(node);},ownerDocument:{createElement(){return {className:'',textContent:'',setAttribute(){}};}}};
 let session={key:'1:17',ready:true};const mounts=[],opens=[],downloads=[];let refreshes=0,destroys=0,reads=0;
 const backend={listProjects:async()=>{reads++;return [];},loadById:async id=>({databaseId:id}),listAuditEvents:async()=>[],loadStatisticsPreferences:async()=>({state:'loaded',revision:0,settings:null,draft:null,location:'local',ownerToken:'test-owner'}),saveStatisticsPreferences:async()=>({state:'saved',revision:1}),saveStatisticsPreferencesDraft:async()=>({savedAt:'2026-11-17T00:00:00Z'}),clearStatisticsPreferencesDraft:async()=>({cleared:true}),...overrides};
 const providerFactory=invoke=>async()=>({projects:await invoke('production_list_projects',{}),source:{kind:'host'}});
 const controller=create({container,backend,getSession:()=>session,providerFactory,mount:(root,options)=>{mounts.push(options);return {refresh:async()=>{refreshes++;},destroy(){destroys++;},...panelOptions};},openProject:async(id,uid)=>opens.push([id,uid]),download:async(...args)=>{downloads.push(args);return 'exports/file.csv';},print:async()=>{}});
 return {controller,container,mounts,opens,downloads,setSession:s=>session=s,counts:()=>({refreshes,destroys,reads})};
}
test('statistics mounts saved host data only and forwards project/stage navigation',async()=>{const s=setup();await s.controller.show();assert.equal(s.mounts.length,1);assert.equal(s.mounts[0].data.source.kind,'host');assert.equal(s.mounts[0].storageKey,'production-cycle-statistics:v1:17');await s.mounts[0].onOpen({projectId:9,stageUid:'stage-a'});assert.deepEqual(s.opens,[[9,'stage-a']]);await s.mounts[0].onDownload('a;b','report.csv','text/csv');assert.deepEqual(s.downloads,[['a;b','report.csv','text/csv']]);});
test('repeated tab entry refreshes without creating a second dashboard',async()=>{const s=setup();await s.controller.show();await s.controller.show();assert.equal(s.mounts.length,1);assert.equal(s.counts().refreshes,1);});
test('two clicks while loading share one request and one mount',async()=>{let release;const gate=new Promise(resolve=>release=resolve);const s=setup({listProjects:()=>gate});const first=s.controller.show(),second=s.controller.show();release([]);await Promise.all([first,second]);assert.equal(s.mounts.length,1);});
test('logout during pending load discards old-session data',async()=>{let release;const gate=new Promise(resolve=>release=resolve);const s=setup({listProjects:()=>gate});const task=s.controller.show();s.setSession({key:'2:17',ready:false});s.controller.invalidate();release([{projectId:99}]);await task;assert.equal(s.mounts.length,0);assert.equal(s.container.children.length,0);});
test('a changed profile destroys the panel and old callbacks cannot open a project',async()=>{const s=setup();await s.controller.show();const old=s.mounts[0];s.setSession({key:'2:25',ready:true});s.controller.invalidate();await s.controller.show();assert.equal(s.counts().destroys,1);assert.equal(s.mounts[1].storageKey,'production-cycle-statistics:v1:25');await assert.rejects(old.onOpen({projectId:1}),/Сеанс изменился/);assert.equal(s.opens.length,0);});
test('failed initial read shows an error and never substitutes demo data',async()=>{const s=setup({listProjects:async()=>{throw Error('SMB unavailable');}});await s.controller.show();assert.equal(s.mounts.length,0);assert.match(s.container.children[0].textContent,/SMB unavailable/);});
test('Statistics is the immediate accessible tab after Gantt and shares keyboard navigation',()=>{const root=path.join(__dirname,'..'),html=fs.readFileSync(path.join(root,'frontend/index.html'),'utf8'),app=fs.readFileSync(path.join(root,'frontend/ui/app.js'),'utf8');assert.match(html,/<button[^>]+id="ganttTab"[^>]*>Диаграмма Ганта<\/button><button[^>]+id="statisticsTab"[^>]*>Статистика<\/button>/);assert.match(html,/id="statisticsView"[^>]+role="tabpanel"[^>]+aria-labelledby="statisticsTab"/);assert.match(app,/\['registry','deadlines','gantt','statistics','admin'\]/);assert.match(app,/if\(view==='statistics'\)void showStatistics\(\)/);});

test('native preferences adapter carries the loaded owner and revision through confirmed and draft writes',async()=>{
 const calls=[],settings={version:2,columns:1,widgets:[]},loaded={state:'loaded',revision:7,settings,draft:null,location:'shared-folder',ownerToken:'workspace-A/profile-A/session-1'};
 const s=setup({loadStatisticsPreferences:async()=>loaded,saveStatisticsPreferences:async(...args)=>{calls.push(['save',...args]);return{state:'saved',revision:8,settings};},saveStatisticsPreferencesDraft:async(...args)=>{calls.push(['draft',...args]);return{savedAt:'2026-11-17T00:00:00Z'};},clearStatisticsPreferencesDraft:async(...args)=>calls.push(['clear',...args])});
 await s.controller.show();const adapter=s.mounts[0].settingsAdapter;
 assert.deepEqual(await adapter.load(),loaded);await adapter.save(JSON.stringify(settings),{expectedRevision:7});await adapter.saveDraft(JSON.stringify(settings),{expectedRevision:7});await adapter.clearDraft();
 assert.deepEqual(calls,[['save',settings,7,loaded.ownerToken],['draft',settings,7,loaded.ownerToken],['clear',loaded.ownerToken]]);
});
test('a native preference response arriving after logout is rejected',async()=>{
 let release;const gate=new Promise(resolve=>release=resolve),s=setup({loadStatisticsPreferences:()=>gate});await s.controller.show();
 const pending=s.mounts[0].settingsAdapter.load();s.setSession({key:'1:17',ready:false});s.controller.invalidate();release({state:'loaded',revision:3,settings:{version:2},ownerToken:'old-owner'});
 await assert.rejects(pending,/Сеанс изменился/);assert.equal(s.container.children.length,0);
});
test('old profile cannot save preferences or drafts after the next profile mounts',async()=>{
 let writes=0;const s=setup({saveStatisticsPreferences:async()=>writes++,saveStatisticsPreferencesDraft:async()=>writes++,clearStatisticsPreferencesDraft:async()=>writes++});await s.controller.show();const old=s.mounts[0].settingsAdapter;await old.load();
 s.setSession({key:'2:25',ready:true});s.controller.invalidate();await s.controller.show();
 await assert.rejects(old.save('{}',{expectedRevision:1}),/Сеанс изменился/);await assert.rejects(old.saveDraft('{}',{expectedRevision:1}),/Сеанс изменился/);await assert.rejects(old.clearDraft(),/Сеанс изменился/);assert.equal(writes,0);
});
test('a save completed after logout cannot show a successful result in the new session',async()=>{
 let release;const gate=new Promise(resolve=>release=resolve),s=setup({saveStatisticsPreferences:()=>gate});await s.controller.show();const adapter=s.mounts[0].settingsAdapter;await adapter.load();
 const pending=adapter.save('{"version":2}',{expectedRevision:0});s.setSession({key:'2:25',ready:true});s.controller.invalidate();release({state:'saved',revision:1});await assert.rejects(pending,/Сеанс изменился/);
});
test('opening a project waits for the layout leave decision and honours Continue editing',async()=>{
 let allowed=false,requests=0;const s=setup({}, {requestLeave:async()=>{requests++;return allowed;},isEditing:()=>true});await s.controller.show();assert.equal(s.controller.isEditing(),true);
 assert.equal(await s.mounts[0].onOpen({projectId:9}),false);assert.deepEqual(s.opens,[]);allowed=true;assert.equal(await s.mounts[0].onOpen({projectId:9,stageUid:'x'}),true);assert.deepEqual(s.opens,[[9,'x']]);assert.equal(requests,2);
});
test('show awaits the native settings ready promise before declaring the panel ready',async()=>{
 let release,finished=false;const ready=new Promise(resolve=>release=resolve),s=setup({}, {ready});const pending=s.controller.show().then(()=>finished=true);await new Promise(resolve=>setImmediate(resolve));assert.equal(s.mounts.length,1);assert.equal(finished,false);release();await pending;assert.equal(finished,true);
});
