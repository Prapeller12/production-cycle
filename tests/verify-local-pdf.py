"""Inspect PDFs produced from the actual packaged Fixed WebView2 window."""
import json,re,sys
from pathlib import Path
import fitz
root=Path(sys.argv[1]);results=[]
clean=lambda text:re.sub(r'\s|\u00ad','',text)
for path in sorted(root.glob('*.pdf')):
    doc=fitz.open(path);texts=[p.get_text()for p in doc]
    assert len(doc)and all(t.strip()for t in texts),(path,'Blank page')
    if 'report-' in path.stem:
        count=45 if 'many' in path.stem else 9;text=clean('\n'.join(texts))
        assert 'Отчётпопроизводственномупроекту' in text,(path,'Missing title')
        assert '167-001' in clean(texts[0]),(path,'Table moved off the first page')
        for i in range(1,count+1):assert '167-'+str(i).zfill(3)in text,(path,'Missing stage',i)
        assert '31.08.2026'in text,(path,'Incomplete date')
        dates=[s['size']for p in doc for b in p.get_text('dict')['blocks']for l in b.get('lines',[])for s in l['spans']if s['text'].strip()in('2026','31.08.2026')]
        assert dates and min(dates)>=13.4,(path,'Date font below retained 14pt')
        for page in doc:
            words=page.get_text('words')
            for i,a in enumerate(words):
                assert a[0]>=10 and a[2]<=page.rect.width-10,(path,'Text outside paper',a)
                for b in words[i+1:]:
                    dx=min(a[2],b[2])-max(a[0],b[0]);dy=min(a[3],b[3])-max(a[1],b[1]);h=min(a[3]-a[1],b[3]-b[1])
                    assert not(dx>1.3 and h>0 and dy/h>.6),(path,'Overlapping words',a,b)
        assert(doc[0].rect.width>doc[0].rect.height)==('landscape'in path.stem),(path,'Wrong orientation')
    results.append({'file':path.name,'pages':len(doc),'noBlankPages':True,'engine':'packaged Fixed WebView2'})
assert len(results)==7,('Missing print formats',len(results))
(root/'pdf-verification.json').write_text(json.dumps(results,ensure_ascii=False,indent=2))
print('PASS seven print formats with no empty pages; report dates and words checked for size, clipping and overlap')
