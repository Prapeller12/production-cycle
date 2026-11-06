"""Build the offline setup manual in DOCX and self-contained HTML from one source."""
import argparse
import html
import json
from pathlib import Path
from docx import Document
from docx.shared import Inches, Pt, RGBColor
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.enum.table import WD_TABLE_ALIGNMENT, WD_CELL_VERTICAL_ALIGNMENT

parser = argparse.ArgumentParser()
parser.add_argument('--version', default='development')
parser.add_argument('--out', type=Path, default=Path('dist/server-manual'))
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
data = json.loads((root / 'docs/server-setup-guide.json').read_text(encoding='utf-8'))
args.out.mkdir(parents=True, exist_ok=True)
doc = Document()
section = doc.sections[0]
section.page_width = Inches(8.5)
section.page_height = Inches(11)
section.top_margin = section.bottom_margin = Inches(.65)
section.left_margin = section.right_margin = Inches(.7)
section.header_distance = section.footer_distance = Inches(.25)
for name in ['Normal', 'Title', 'Subtitle', 'Heading 1', 'Heading 2', 'Header', 'Footer']:
    style = doc.styles[name]
    style.font.name = 'Arial'
    style.font.color.rgb = RGBColor(0, 0, 0)
    if name in ['Title', 'Subtitle']: style.font.italic = False
    style.element.get_or_add_rPr().rFonts.set(qn('w:hAnsi'), 'Arial')
for style in doc.styles:
    for border in list(style.element.iter(qn('w:pBdr'))): border.getparent().remove(border)
doc.styles['Normal'].font.size = Pt(11.5)
doc.styles['Normal'].paragraph_format.space_after = Pt(6)
doc.styles['Normal'].paragraph_format.line_spacing = 1.05
doc.styles['Title'].font.size = Pt(24)
doc.styles['Subtitle'].font.size = Pt(15)
doc.styles['Heading 1'].font.size = Pt(17)
doc.styles['Heading 1'].paragraph_format.space_after = Pt(12)
doc.styles['Heading 1'].paragraph_format.space_before = Pt(0)
doc.core_properties.title = data['title'] + ' ' + data['subtitle']
doc.core_properties.author = 'Производственный цикл'
doc.core_properties.subject = 'Автономный запуск и настройка доступа'
header = section.header.paragraphs[0]
header.text = 'Производственный цикл    |    Настройка и запуск'
header.runs[0].font.size = Pt(9)
footer = section.footer.paragraphs[0]
footer.text = f'Версия {args.version}    •    '
for field_name in ['PAGE', 'NUMPAGES']:
    if field_name == 'NUMPAGES': footer.add_run(' / ')
    field = OxmlElement('w:fldSimple')
    field.set(qn('w:instr'), field_name)
    footer._p.append(field)
for run in footer.runs: run.font.size = Pt(9)

def paragraph(text, bold=False, size=None):
    p = doc.add_paragraph()
    run = p.add_run(text)
    run.bold = bold
    if size: run.font.size = Pt(size)
    p.paragraph_format.widow_control = True
    return p

def table(spec):
    t = doc.add_table(rows=1, cols=len(spec['headers']))
    t.alignment = WD_TABLE_ALIGNMENT.CENTER
    t.autofit = False
    widths = [2.4, 4.7]
    for column, width in zip(t.columns, widths): column.width = Inches(width)
    for i, name in enumerate(spec['headers']): t.rows[0].cells[i].text = name
    header_property = OxmlElement('w:tblHeader');t.rows[0]._tr.get_or_add_trPr().append(header_property)
    for row in spec['rows']:
        cells = t.add_row().cells
        for i, text in enumerate(row): cells[i].text = text
    for row_index, row in enumerate(t.rows):
        no_split = OxmlElement('w:cantSplit');row._tr.get_or_add_trPr().append(no_split)
        for i, cell in enumerate(row.cells):
            cell.width = Inches(widths[i])
            cell.vertical_alignment = WD_CELL_VERTICAL_ALIGNMENT.CENTER
            props = cell._tc.get_or_add_tcPr()
            borders = OxmlElement('w:tcBorders')
            for edge in ['top', 'left', 'bottom', 'right']:
                border = OxmlElement('w:' + edge)
                for attr, value in [('val', 'single'), ('sz', '5'), ('color', 'D9D9D9')]: border.set(qn('w:' + attr), value)
                borders.append(border)
            props.append(borders)
            margins = OxmlElement('w:tcMar')
            for edge, value in [('top', '90'), ('bottom', '90'), ('left', '110'), ('right', '110')]:
                margin = OxmlElement('w:' + edge);margin.set(qn('w:w'), value);margin.set(qn('w:type'), 'dxa');margins.append(margin)
            props.append(margins)
            if row_index == 0 or row_index % 2 == 0:
                shade = OxmlElement('w:shd');shade.set(qn('w:fill'), 'E7EDF3' if row_index == 0 else 'F6F8FA');props.append(shade)
            for p in cell.paragraphs:
                p.paragraph_format.space_after = Pt(2)
                for run in p.runs:
                    run.font.size = Pt(10.5)
                    run.bold = row_index == 0
    doc.add_paragraph().paragraph_format.space_after = Pt(1)

parts = ['<!doctype html><html lang="ru"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Производственный цикл Настройка</title>',
 '<style>body{font:18px/1.5 Arial,sans-serif;max-width:960px;margin:32px auto;padding:0 24px;color:#111;background:white}h1,h2{color:black;line-height:1.2}h2{margin-top:48px}table{border-collapse:collapse;width:100%;margin:16px 0}td,th{border:1px solid #d9d9d9;padding:12px;text-align:left}th{background:#e7edf3}tr:nth-child(even){background:#f6f8fa}li{margin-bottom:18px}.result{display:block;margin-top:5px}p{overflow-wrap:anywhere}@media print{body{font-size:11pt;margin:0}h2{break-before:page}tr,li{break-inside:avoid}}</style>',
 f'<h1>{html.escape(data["title"])}</h1><p>{html.escape(data["subtitle"])}</p><p>{html.escape(data["date"])} · Версия {html.escape(args.version)}</p>']
for index, page in enumerate(data['sections']):
    if index == 0:
        doc.add_paragraph(data['title'], 'Title')
        doc.add_paragraph(data['subtitle'], 'Subtitle')
        paragraph(f'{data["date"]}    Версия {args.version}', size=10)
    else: doc.add_page_break()
    doc.add_heading(page['title'], level=1)
    parts.append('<h2>' + html.escape(page['title']) + '</h2>')
    for text in page.get('paragraphs', []): paragraph(text);parts.append('<p>' + html.escape(text) + '</p>')
    if 'steps' in page:
        parts.append('<ol>')
        for number, (action, result) in enumerate(page['steps'], 1):
            p = paragraph(f'{number}. {action}', bold=True)
            p.paragraph_format.keep_with_next = True
            p = paragraph('Результат: ' + result)
            p.paragraph_format.left_indent = Inches(.18)
            parts.append('<li><strong>' + html.escape(action) + '</strong><span class="result">Результат: ' + html.escape(result) + '</span></li>')
        parts.append('</ol>')
    if 'table' in page:
        table(page['table'])
        spec = page['table']
        parts.append('<table><thead><tr>' + ''.join('<th>' + html.escape(x) + '</th>' for x in spec['headers']) + '</tr></thead><tbody>')
        for row in spec['rows']: parts.append('<tr>' + ''.join('<td>' + html.escape(x) + '</td>' for x in row) + '</tr>')
        parts.append('</tbody></table>')
    for text in page.get('after', []): paragraph(text);parts.append('<p>' + html.escape(text) + '</p>')
parts.append('</html>')
doc.save(args.out / 'ProductionCycle_Setup_Guide.docx')
(args.out / 'ProductionCycle_Setup_Guide.html').write_text('\n'.join(parts), encoding='utf-8')
print(json.dumps({'docx':str(args.out / 'ProductionCycle_Setup_Guide.docx'),'html':str(args.out / 'ProductionCycle_Setup_Guide.html'),'chapters':len(data['sections'])}))
