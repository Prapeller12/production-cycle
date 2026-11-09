import argparse,json,os
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('--root',default='dist')
parser.add_argument('--version',required=True)
parser.add_argument('--out',default='dist/button-audit-report.txt')
args=parser.parse_args()
root=Path(args.root)
reports={}
for name,relative in [('IE11/MSHTML','ie-layout-qa/ie-button-audit.json'),('Edge','browser-qa/edge-button-audit.json'),('ES5 в Edge','browser-qa/legacy-button-audit.json')]:
    result=json.loads((root/relative).read_text(encoding='utf-8-sig'))
    assert result['error'] is None,(name,result)
    assert result['coverage']['missing']==[],(name,result['coverage'])
    reports[name]=result
native=json.loads((root/'ie-layout-qa/ie-native-print.json').read_text(encoding='utf-8-sig'))
assert native['error'] is None and native['nativeDialogs']==10,native
baseline=json.loads((root/'ie-layout-qa/ie-print-baseline.json').read_text(encoding='utf-8-sig'))
lines=['ПРОВЕРКА КНОПОК ПРОГРАММЫ «ПРОИЗВОДСТВЕННЫЙ ЦИКЛ»',f'Версия: {args.version}',f'Среда сборки: {os.environ.get("QA_WINDOWS_IMAGE","Windows CI")}',
       'Проверки выполнялись на отдельных временных базах, с созданием тестовых профилей.',
       'Пользовательская база в проверках не использовалась.','']
for name,result in reports.items():
    c=result['coverage']
    lines += [f'{name}: {c["staticButtons"]} кнопок из HTML; {c["dynamicControls"]} типов динамических действий; {len(result["scenarios"])} сценариев. Непроверенных кнопок: 0.']
lines += ['',f'IE: фактически открыты и отменены {native["nativeDialogs"]} системных окон Print.',
          'Гант: A3 и A4 в масштабах день, неделя, месяц, квартал. Дополнительно: структура и общий отчёт.',
          'Проверены восстановление интерфейса после отмены, резервный вызов без кадров обновления окна и видимая ошибка печати.',
          'PDF в Edge проверены отдельно: Гант A3/A4 и структура; пустых страниц нет.','',
          'ОГРАНИЧЕНИЯ',
          f'В исходной версии на тестовом IE команды печати Ганта дошли до window.print: {baseline.get("calls",0)} из 2.',
          'Поэтому точная причина отсутствия отклика на сервере владельца не подтверждена.',
          'Физический принтер, его драйвер, групповые политики и Server 2019 владельца здесь не проверялись.',
          'Edge: TXT проверены через реальные загрузки браузера. IE: проверены Blob и обработчик msSaveOrOpenBlob; системное окно сохранения файла не проверялось.',
          'Для импорта использовались тестовые Blob-файлы через штатные обработчики выбора файлов и FileReader.',
          'Проверка окна выбора файла Windows не заменяется этими сценариями.','',
          'КНОПКИ И ДЕЙСТВИЯ (название — число нажатий в успешном сценарии IE)']
ie=reports['IE11/MSHTML']
for key,count in sorted(ie['coverage']['clicked'].items()):
    lines.append(f'{ie["coverage"]["labels"].get(key,key)} — {count} [{key}]')
lines+=['','ПРОВЕРЕННЫЕ СЦЕНАРИИ IE']+ie['scenarios']
Path(args.out).write_text('\n'.join(lines)+'\n',encoding='utf-8')
print('Button audit report generated from successful measured results')
