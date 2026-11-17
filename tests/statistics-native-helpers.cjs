'use strict';
// Shared by Windows-only runners. Every data/preference operation reaches real native IPC.
const assert=require('node:assert/strict');
const S='#statisticsMount ';
const action=(name,widget)=>S+'[data-action="'+name+'"]'+(widget?'[data-widget="'+widget+'"]':'');
const widget=id=>S+'.pc-widget[data-widget="'+id+'"]';
async function openStatistics(page){
 await page.evaluate(()=>document.querySelector('[data-close="onboardingModal"]')?.click());
 await page.click('#statisticsTab');
 await page.waitForSelector(widget('participation'));
 await page.waitForFunction(()=>!document.querySelector('#statisticsMount [data-action="settings"]')?.disabled);
}
async function edit(page){await page.click(action('settings'));await page.waitForSelector(action('save-settings'));}
async function save(page){await page.click(action('save-settings'));await page.waitForSelector(action('save-settings'),{state:'detached'});}
async function preferences(page){return page.evaluate(()=>window.__TAURI__.core.invoke('statistics_preferences_load'));}
async function login(page,userId){
 await page.evaluate(()=>document.querySelector('[data-close="onboardingModal"]')?.click());
 if(!(await page.locator('#networkLoginModal').evaluate(e=>e.classList.contains('open'))))await page.click('#networkLogoutBtn');
 await page.waitForFunction(()=>document.getElementById('networkLoginModal').classList.contains('open')&&!document.getElementById('networkLoginBtn').disabled);
 await page.locator('#networkLoginUser').selectOption(String(userId));await page.fill('#networkLoginPin','739201');await page.click('#networkLoginBtn');
 await page.waitForFunction(()=>!document.getElementById('networkLoginModal').classList.contains('open'));
 await page.evaluate(()=>document.querySelector('[data-close="onboardingModal"]')?.click());
}
async function showAllWidgets(page){
 await edit(page);
 for(const id of ['participation','enterprises','portfolio','calendar','executors','risks','statuses','events','quality']){
  await page.locator(S+'[data-widget-toggle="'+id+'"]').check();
  await page.locator(S+'[data-widget-width="'+id+'"]').selectOption('half');
 }
 await page.locator(S+'[data-setting="columns"]').selectOption('2');
 await save(page);
}
async function seed(page){
 return page.evaluate(async()=>{
  const B=Production.backend,admin=(await B.listAuditUsers()).find(u=>u.role==='admin');
  const order='STAT-QA-20-100',largeOrder='STAT-QA-100',projects=[];
  const project=(name,orderNo,enterprise)=>({name,orderNo,initiator:'Проверка статистики',executor:'Координатор QA',addressees:enterprise,start:'2026-09-01',deadline:'2026-12-31'});
  const stage=(i,executor)=>({uid:'statistics-stage-'+i,seq:i+1,sort:i+1,parentUid:null,title:'Проверочный этап '+(i+1),executor,addressees:'Предприятие QA',start:'2026-09-01',deadline:'2026-11-10',status:'work',comment:'Полный комментарий '+(i+1)});
  const sign={userId:admin.id,pin:'739201',comment:'Учебные данные native Statistics QA'};
  for(let i=0;i<5;i++){
   const saved=await B.saveSigned({databaseId:null,project:project('Проект статистики '+(i+1),order,'Предприятие QA '+(i+1)),stages:[stage(i,'Исполнитель QA '+(i+1))],nextSeq:i+2,collapsed:{}},sign);projects.push(saved.projectId);
  }
  const completed=await B.loadById(projects[0]);completed.stages[0].status='done';
  await B.saveSigned(completed,{...sign,comment:'Выполнен единственный этап первого исполнителя',evidence:{documentType:'Акт приёмки',documentReference:'STAT-QA-ACT',comment:'Один этап из пяти',stageUids:[completed.stages[0].uid]}});
  const large=await B.saveSigned({databaseId:null,project:project('Сто этапов статистики',largeOrder,'Предприятие QA 100'),stages:Array.from({length:100},(_,i)=>stage(i,'Массовый исполнитель '+(i%4+1))),nextSeq:101,collapsed:{}},sign);
  return {order,largeOrder,projects,largeProjectId:large.projectId,users:await B.listAuditUsers()};
 });
}
async function setFilter(page,name,value){await page.locator(S+'[data-filter="'+name+'"]').selectOption(value);}
async function width(page,id,value){await edit(page);await page.locator(S+'[data-widget-width="'+id+'"]').selectOption(value);await save(page);}
async function expectSaved(page){const p=await preferences(page);assert.equal(p.state,'loaded');assert.equal(p.settings?.version,2);assert.ok(p.revision>=1);return p;}
module.exports={S,action,widget,openStatistics,edit,save,preferences,login,showAllWidgets,seed,setFilter,width,expectSaved};
