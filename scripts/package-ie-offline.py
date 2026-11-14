"""Full offline release from the exact delivered package and the tested interface."""
import argparse,hashlib,json,shutil,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BASE_SHA='6592f83b4a62bd217b75d04020c8f010ac480b580b0d59c83074b886748a8331'
def sha(data):return hashlib.sha256(data).hexdigest()
p=argparse.ArgumentParser();p.add_argument('--base',type=Path,required=True);p.add_argument('--frontend',type=Path,required=True);p.add_argument('--source',required=True);a=p.parse_args()
assert sha(a.base.read_bytes())==BASE_SHA,'Unverified previous full ZIP'
out=ROOT/'dist/ie-offline-release';out.mkdir(parents=True,exist_ok=True);target=out/'ProductionCycleServer'
if target.exists():shutil.rmtree(target)
with zipfile.ZipFile(a.base)as z:
 assert z.testzip()is None;z.extractall(out)
 old=json.loads(z.read('ProductionCycleServer/package-manifest.json'))
 for f in old['files']:assert sha(z.read('ProductionCycleServer/'+f['path']))==f['sha256'],f['path']
shutil.rmtree(target/'frontend');shutil.copytree(a.frontend,target/'frontend')
for name in ['index.html','legacy/index.html']:
 assert '/compat/browser-entry.js' in (target/'frontend'/name).read_text(encoding='utf-8')
for name in ['ui/app.js','legacy/app.js']:
 assert 'data-act="copy"' in (target/'frontend'/name).read_text(encoding='utf-8')
template=json.loads((ROOT/'server/resources/project-import-template.json').read_text())
assert sha((target/template['fileName']).read_bytes())==template['sha256']
assert template['base64'] in (target/'frontend/config/import-template.js').read_text()
(target/'ИСПРАВЛЕНИЯ_IE_И_EXCEL.txt').write_text((ROOT/'docs/ie-offline-update.txt').read_text(encoding='utf-8'),encoding='utf-8-sig')
(target/'VERSION.txt').write_text('server-ie-offline-'+a.source[:7]+'\nserver-base=14fbf50\n',encoding='ascii')
(target/'package-manifest.json').unlink()
files=[]
for f in sorted(target.rglob('*')):
 if not f.is_file():continue
 name=f.relative_to(target).as_posix();assert not name.startswith(('server-data/','tls/')) and not name.endswith(('.db','.pem','.pfx')),name
 files.append({'path':name,'bytes':f.stat().st_size,'sha256':sha(f.read_bytes())})
manifest={'version':'server-ie-offline-'+a.source[:7],'source':a.source,'baseZipSha256':BASE_SHA,'serverBase':'14fbf50','excelTemplateSha256':template['sha256'],'files':files}
(target/'package-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
zip_path=out/('ProductionCycle-server-IE-Excel-'+a.source[:7]+'-windows-x64.zip')
with zipfile.ZipFile(zip_path,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
 for f in sorted(target.rglob('*')):
  if f.is_file():z.write(f,f.relative_to(out).as_posix())
with zipfile.ZipFile(a.base)as old,zipfile.ZipFile(zip_path)as new:
 assert new.testzip()is None
 for name in old.namelist():
  if '/frontend/'not in name and not name.endswith(('VERSION.txt','package-manifest.json')):assert old.read(name)==new.read(name),name
digest=sha(zip_path.read_bytes());zip_path.with_suffix('.zip.sha256').write_text(digest+'\n',encoding='ascii')
print(json.dumps({'path':str(zip_path),'bytes':zip_path.stat().st_size,'sha256':digest,'serverAndNetworkFilesUnchanged':True}))
