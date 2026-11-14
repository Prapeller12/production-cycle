"""Package static assets only; never include server settings or user data."""
import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
parser.add_argument('--out', default='dist/frontend-update')
parser.add_argument('--kind', choices=['IE-layout', 'buttons'], default='IE-layout')
parser.add_argument('--qa-report')
args = parser.parse_args()
repo = Path(__file__).resolve().parent.parent
frontend = repo / 'frontend'
assert (frontend / 'legacy/app.js').is_file(), 'Run npm run build:legacy first'
out = repo / args.out
out.mkdir(parents=True, exist_ok=True)
archive = out / f'ProductionCycle-{args.kind}-{args.version}.zip'
files = sorted(p for p in frontend.rglob('*') if p.is_file())
assert not any(p.suffix.lower() in {'.exe', '.db', '.sqlite', '.pem', '.key'} for p in files)
manifest = {
    'version': args.version,
    'scope': 'frontend-only',
    'compatible_releases': ['browser-server-b7dbce3', 'shared-folder-4c84ac6'],
    'files': {p.relative_to(repo).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
}
instructions = (repo / 'docs/ie-layout-update.txt').read_text(encoding='utf-8')
with ZipFile(archive, 'w', ZIP_DEFLATED, compresslevel=9) as z:
    for p in files:
        z.write(p, p.relative_to(repo).as_posix())
    z.writestr('ОБНОВЛЕНИЕ.txt', '\ufeff' + instructions)
    z.writestr('frontend-update-manifest.json', json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    if args.qa_report:
        z.writestr('ПРОВЕРКА_КНОПОК.txt', '\ufeff' + Path(args.qa_report).read_text(encoding='utf-8'))
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_suffix('.zip.sha256').write_text(f'{digest}  {archive.name}\n', encoding='ascii')
print(json.dumps({'file': str(archive), 'sha256': digest, 'assets': len(files)}))
