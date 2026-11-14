"""Full release preserving the exact server, database settings, Excel sample and guides."""
import argparse,hashlib,json,shutil,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BASE_SHA='b8225908d7e747aab04b46f425b41a5c94d3b933183fc14dcdc5636b60a7a63c'
def sha(data):return hashlib.sha256(data).hexdigest()
p=argparse.ArgumentParser();p.add_argument('--base',type=Path,required=True);p.add_argument('--frontend',type=Path,required=True);p.add_argument('--source',required=True);a=p.parse_args()
assert sha(a.base.read_bytes())==BASE_SHA,'Unverified previous full ZIP'
out=ROOT/'dist/ie-report-release';out.mkdir(parents=True,exist_ok=True);target=out/'ProductionCycleServer'
if target.exists():shutil.rmtree(target)
with zipfile.ZipFile(a.base)as z:
 assert z.testzip()is None;z.extractall(out)
 old=json.loads(z.read('ProductionCycleServer/package-manifest.json'))
 for f in old['files']:assert sha(z.read('ProductionCycleServer/'+f['path']))==f['sha256'],f['path']
shutil.rmtree(target/'frontend');shutil.copytree(a.frontend,target/'frontend')
for name in ['ui/app.js','legacy/app.js']:
 text=(target/'frontend'/name).read_text(encoding='utf-8');assert 'report-date-part'in text and 'report-project'in text and 'report-summary'in text and 'data-act="copy"'in text
assert (target/'frontend/styles/report-print.css').is_file()
(target/'ИСПРАВЛЕНИЕ_ПЕЧАТИ_IE.txt').write_text((ROOT/'docs/ie-report-print-update.txt').read_text(encoding='utf-8'),encoding='utf-8-sig')
(target/'VERSION.txt').write_text('server-ie-report-'+a.source[:7]+'\nserver-base=14fbf50\n',encoding='ascii')
(target/'package-manifest.json').unlink();files=[]
for f in sorted(target.rglob('*')):
 if not f.is_file():continue
 name=f.relative_to(target).as_posix();assert not name.startswith(('server-data/','tls/'))and not name.endswith(('.db','.pem','.pfx')),name
 files.append({'path':name,'bytes':f.stat().st_size,'sha256':sha(f.read_bytes())})
manifest={'version':'server-ie-report-'+a.source[:7],'source':a.source,'baseZipSha256':BASE_SHA,'serverBase':'14fbf50','files':files}
(target/'package-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
zip_path=out/('ProductionCycle-server-IE-print-'+a.source[:7]+'-windows-x64.zip')
with zipfile.ZipFile(zip_path,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
 for f in sorted(target.rglob('*')):
  if f.is_file():z.write(f,f.relative_to(out).as_posix())
with zipfile.ZipFile(a.base)as old,zipfile.ZipFile(zip_path)as new:
 assert new.testzip()is None
 for name in old.namelist():
  if '/frontend/'not in name and not name.endswith(('VERSION.txt','package-manifest.json')):assert old.read(name)==new.read(name),name
digest=sha(zip_path.read_bytes());zip_path.with_suffix('.zip.sha256').write_text(digest+'\n',encoding='ascii')
print(json.dumps({'path':str(zip_path),'bytes':zip_path.stat().st_size,'sha256':digest,'serverAndNetworkFilesUnchanged':True}))
