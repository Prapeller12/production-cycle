"""Mutate the shipped workbook into parser regression inputs (no Office dependency)."""
import base64
import copy
import io
import json
from pathlib import Path
import zipfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
NS = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
ET.register_namespace('', NS)
source = base64.b64decode(json.loads((ROOT/'server/resources/project-import-template.json').read_text())['base64'])
with zipfile.ZipFile(io.BytesIO(source)) as archive:
    original = {name: archive.read(name) for name in archive.namelist()}
fixtures = {}

def make(name, change):
    files = copy.deepcopy(original)
    trees = {p: ET.fromstring(files[p]) for p in ('xl/worksheets/sheet1.xml', 'xl/worksheets/sheet2.xml', 'xl/workbook.xml')}
    change(trees, files)
    for p, tree in trees.items():
        if p in files:
            files[p] = ET.tostring(tree, encoding='utf-8', xml_declaration=True)
    out = io.BytesIO()
    with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED) as archive:
        for p, data in files.items():
            archive.writestr(p, data)
    fixtures[name] = base64.b64encode(out.getvalue()).decode()

def cell(tree, address, value, typ='str'):
    c = tree.find('.//{%s}c[@r="%s"]' % (NS, address))
    c.set('t', typ)
    for child in list(c):
        c.remove(child)
    ET.SubElement(c, '{%s}v' % NS).text = str(value)

def stage_change(address, value):
    return lambda t, _: cell(t['xl/worksheets/sheet2.xml'], address, value)

make('duplicate', stage_change('B9', 'Э01'))
make('missingParent', stage_change('C9', 'НЕТ'))
make('selfParent', stage_change('C8', 'Э01'))
make('cycle', stage_change('C8', 'Э02'))
make('badDate', stage_change('G8', '31.02.2027'))
make('emptyTitle', stage_change('D8', ''))
make('markup', stage_change('D8', '<img src=x onerror=alert(1)>'))
make('missingSheet', lambda _, f: f.pop('xl/worksheets/sheet2.xml'))
def big_dimensions(t, _):
    tree = t['xl/worksheets/sheet2.xml']
    element = tree.find('{%s}dimension' % NS)
    if element is None:
        element = ET.Element('{%s}dimension' % NS)
        tree.insert(0, element)
    element.set('ref', 'A1:XFD1048576')
make('bigDimensions', big_dimensions)
def formula(t, _):
    c = t['xl/worksheets/sheet2.xml'].find('.//{%s}c[@r="D8"]' % NS)
    ET.SubElement(c, '{%s}f' % NS).text = '1+1'
make('formula', formula)
def duplicate_header(t, _):
    tree = t['xl/worksheets/sheet2.xml']
    row = tree.find('.//{%s}row[@r="7"]' % NS)
    c = ET.SubElement(row, '{%s}c' % NS, {'r': 'K7', 't': 'str'})
    ET.SubElement(c, '{%s}v' % NS).text = 'Код этапа *'
make('duplicateHeader', duplicate_header)
def epoch1904(t, _):
    book = t['xl/workbook.xml']
    properties = book.find('{%s}workbookPr' % NS)
    if properties is None:
        properties = ET.Element('{%s}workbookPr' % NS)
        book.insert(0, properties)
    properties.set('date1904', '1')
    for p, addresses in [('xl/worksheets/sheet1.xml', ['C12','C13']), ('xl/worksheets/sheet2.xml', [f'{c}{r}' for r in range(8,13) for c in 'GH'])]:
        for address in addresses:
            c = t[p].find('.//{%s}c[@r="%s"]' % (NS,address))
            v = c.find('{%s}v' % NS)
            v.text = str(int(v.text)-1462)
make('epoch1904', epoch1904)
def reordered(t, _):
    data = t['xl/worksheets/sheet2.xml'].find('{%s}sheetData' % NS)
    rows = [data.find('{%s}row[@r="%d"]' % (NS,n)) for n in range(8,13)]
    for row in rows:
        data.remove(row)
    for n, row in enumerate(reversed(rows), 8):
        row.set('r', str(n))
        for c in row:
            c.set('r', ''.join(ch for ch in c.get('r') if ch.isalpha())+str(n))
        data.append(row)
make('reordered', reordered)
target = ROOT/'dist/excel-fixtures.json'
target.parent.mkdir(exist_ok=True)
target.write_text(json.dumps(fixtures), encoding='utf-8')
print(f'Prepared {len(fixtures)} workbook regression inputs')
