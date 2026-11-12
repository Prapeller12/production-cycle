import json,re,sys
from pathlib import Path
import fitz
root=Path(sys.argv[1]);results=[]
def normalize(text):return re.sub(r'\s|\u00ad','',text)
def overlaps(page):
 words=page.get_text('words');bad=[]
 for i,a in enumerate(words):
  for b in words[i+1:]:
   dx=min(a[2],b[2])-max(a[0],b[0]);dy=min(a[3],b[3])-max(a[1],b[1]);height=min(a[3]-a[1],b[3]-b[1])
   if dx>1.3 and height>0 and dy/height>.6:bad.append([a[:5],b[:5]])
 return bad
def number_position(page,value):
 for block in page.get_text('dict')['blocks']:
  for line in block.get('lines',[]):
   for span in line['spans']:
    if span['text'].strip()==value:return span['bbox']
 raise AssertionError(('Missing KPI',value))
for path in sorted(root.glob('*report-*.pdf'))+sorted(root.glob('native-*.pdf')):
 d=fitz.open(path);texts=[p.get_text()for p in d];assert len(d)and all(t.strip()for t in texts),(path,'Blank page')
 before='before' in path.stem;count=45 if 'many' in path.stem else 9
 text=normalize('\n'.join(texts));assert 'Отчётпопроизводственномупроекту' in text,(path,'Missing report title')
 for i in range(1,count+1):assert '167-'+str(i).zfill(3) in text,(path,'Missing stage',i)
 assert '31.08.2026' in text,(path,'Printed date is incomplete')
 if not before:
  assert '167-001' in normalize(texts[0]),(path,'Table was pushed off the first page')
  k1=number_position(d[0],str(count));k2=number_position(d[0],'4');assert abs(k1[1]-k2[1])<3,(path,'KPIs collapsed into a vertical list',k1,k2)
  for n,page in enumerate(d):
   bad=overlaps(page);assert not bad,(path,'Overlapping printed words',n+1,bad[:8])
   for w in page.get_text('words'):assert w[0]>=10 and w[2]<=page.rect.width-10,(path,'Text exceeds paper',n+1,w)
  years=[s['size']for p in d for b in p.get_text('dict')['blocks']for l in b.get('lines',[])for s in l['spans']if s['text'].strip()in ('2026','31.08.2026')]
  assert years and min(years)>=13.4,(path,'Stage date font shrank below 14pt',years[:8])
  if 'many' in path.stem:assert len(d)>1,(path,'Long report did not paginate')
 else:
  assert '167-001'not in normalize(texts[0]),(path,'Baseline no longer reproduces the tall cover')
 width,height=d[0].rect.width,d[0].rect.height
 assert (width>height)==('landscape'in path.stem),(path,'Unexpected paper orientation',width,height)
 results.append({'file':path.name,'engine':'MSHTML to Windows PDF printer'if path.stem.startswith('native')else'Edge','pages':len(d),'stages':count,'paper':'A4 landscape'if width>height else'A4 portrait','baseline':before,'noBlankPages':True,'noOverlappingWords':not before})
assert len(results)==8,('Missing actual report PDFs',len(results))
(root/'report-pdf-verification.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(results,ensure_ascii=False,indent=2))
