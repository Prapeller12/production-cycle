"""Build a complete offline server distribution, preserving verified launch/setup files."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BASE_HASH = '1c05e7b1c886de20efaa3acc35e9135de0fb904ed584a1e863b5a9f54892e10a'
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
parser.add_argument('--base', type=Path, default=ROOT/'dist/base/server.zip')
args = parser.parse_args()
assert sha(args.base) == BASE_HASH, 'Unverified base package'
output = ROOT/'dist/excel-release'
target = output/'ProductionCycleServer'
if target.exists():
    shutil.rmtree(target)
output.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(args.base) as archive:
    assert archive.testzip() is None
    archive.extractall(output)
old_manifest = json.loads((target/'package-manifest.json').read_text(encoding='utf-8-sig'))
for item in old_manifest['files']:
    assert sha(target/item['path']) == item['sha256'], item['path']
shutil.copyfile(ROOT/'server/target/release/production-cycle-server.exe', target/'Производственный_цикл.exe')
shutil.rmtree(target/'frontend')
shutil.copytree(ROOT/'frontend', target/'frontend')
template = json.loads((ROOT/'server/resources/project-import-template.json').read_text())
(target/template['fileName']).write_bytes(base64.b64decode(template['base64']))
assert sha(target/template['fileName']) == template['sha256']
(target/'ОБНОВЛЕНИЕ_И_ИМПОРТ.txt').write_text((ROOT/'docs/excel-server-update.txt').read_text(encoding='utf-8'), encoding='utf-8-sig')
shutil.copyfile(ROOT/'dist/button-audit-report.txt', target/'ПРОВЕРКА_КНОПОК.txt')
(target/'VERSION.txt').write_text('server-excel-'+args.version+'\n', encoding='ascii')
(target/'package-manifest.json').unlink()
files = []
for path in sorted(target.rglob('*')):
    if not path.is_file():
        continue
    name = path.relative_to(target).as_posix()
    assert not name.startswith(('server-data/', 'tls/')) and not name.endswith(('.db','.pem','.pfx')), name
    files.append({'path':name, 'bytes':path.stat().st_size, 'sha256':sha(path)})
manifest = {'version':'server-excel-'+args.version, 'source':args.version, 'architecture':'local-server-and-browser', 'launcherBase':'shared-folder-4c84ac6', 'baseZipSha256':BASE_HASH, 'excelTemplateSha256':template['sha256'], 'files':files}
(target/'package-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
zip_path = output/f'ProductionCycle-server-excel-{args.version}-windows-x64.zip'
with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for path in sorted(target.rglob('*')):
        if path.is_file():
            archive.write(path, path.relative_to(output).as_posix())
digest = sha(zip_path)
zip_path.with_suffix('.zip.sha256').write_text(digest+'\n', encoding='ascii')
print(json.dumps({'file':zip_path.name, 'bytes':zip_path.stat().st_size, 'sha256':digest}))
