import json
import sys
from pathlib import Path
import fitz

root = Path(sys.argv[1])
results = []
for path in sorted(root.rglob('*.pdf')):
    doc = fitz.open(path)
    assert len(doc), path
    texts = [page.get_text().strip() for page in doc]
    assert all(texts), (path, 'Blank printed page', [i + 1 for i, text in enumerate(texts) if not text])
    expected = 'Структура подчинённости' if 'structure' in path.stem else 'Диаграмма Ганта'
    assert expected in texts[0], (path, 'Report is absent from the first page', texts[0][:200])
    width, height = doc[0].rect.width, doc[0].rect.height
    assert width > height, (path, 'Portrait instead of landscape')
    expected_width = (420 if 'a3' in path.stem else 297) * 72 / 25.4
    assert abs(width - expected_width) < 3, (path, width, expected_width)
    results.append({'file': path.name, 'pages': len(doc), 'first_page_has_report': True, 'blank_pages': 0, 'width_pt': width})
assert results, 'No print artifacts were generated'
print(json.dumps(results, ensure_ascii=False, indent=2))
