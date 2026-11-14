"""Full offline package: verified existing EXE plus checked frontend and current user guide."""
import argparse,hashlib,json,shutil,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
p=argparse.ArgumentParser();p.add_argument('--base',type=Path,required=True);p.add_argument('--frontend',type=Path,required=True);p.add_argument('--guide',type=Path,required=True);p.add_argument('--source',required=True);a=p.parse_args()
base_sha='99ecf6e76a71bd6858d584c86a6101b2a08410bc77cf2f2989c04e2aed037c3b'
assert sha(a.base)==base_sha,'Unverified server base'
out=ROOT/'dist/copy-release';out.mkdir(parents=True,exist_ok=True);target=out/'ProductionCycleServer'
if target.exists():shutil.rmtree(target)
with zipfile.ZipFile(a.base)as z:assert z.testzip()is None;z.extractall(out)
original=json.loads((target/'package-manifest.json').read_text(encoding='utf-8-sig'))
for entry in original['files']:assert sha(target/entry['path'])==entry['sha256'],entry['path']
server_sha=sha(target/'Производственный_цикл.exe')
shutil.rmtree(target/'frontend');shutil.copytree(a.frontend,target/'frontend')
assert (target/'frontend/legacy/app.js').is_file() and (target/'frontend/ui/app.js').is_file()
for file in ['frontend/ui/app.js','frontend/legacy/app.js']:
 text=(target/file).read_text(encoding='utf-8');assert 'data-act="copy"'in text and "querySelector('.notice.ok')"in text,file
shutil.copyfile(a.guide,target/'ProductionCycle_User_Guide.docx')
(target/'КОПИРОВАНИЕ_ЭТАПОВ.txt').write_text((ROOT/'docs/stage-copy-update.txt').read_text(encoding='utf-8'),encoding='utf-8-sig')
(target/'VERSION.txt').write_text('server-stage-copy-'+a.source[:7]+'\nserver-base=14fbf50\n',encoding='ascii')
(target/'package-manifest.json').unlink()
files=[]
for f in sorted(target.rglob('*')):
 if f.is_file():
  name=f.relative_to(target).as_posix();assert not name.startswith(('server-data/','tls/')) and not name.endswith(('.db','.pem','.pfx')),name
  files.append({'path':name,'bytes':f.stat().st_size,'sha256':sha(f)})
assert sha(target/'Производственный_цикл.exe')==server_sha
manifest={'version':'server-stage-copy-'+a.source[:7],'source':a.source,'serverBase':'14fbf50','baseZipSha256':base_sha,'serverExeSha256':server_sha,'userGuideSha256':sha(a.guide),'files':files}
(target/'package-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
zip_path=out/('ProductionCycle-server-copy-'+a.source[:7]+'-windows-x64.zip')
with zipfile.ZipFile(zip_path,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
 for f in sorted(target.rglob('*')):
  if f.is_file():z.write(f,f.relative_to(out).as_posix())
zip_path.with_suffix('.zip.sha256').write_text(sha(zip_path)+'\n',encoding='ascii')
print(json.dumps({'file':str(zip_path),'sha256':sha(zip_path),'bytes':zip_path.stat().st_size,'serverExeUnchanged':True,'source':a.source}))
