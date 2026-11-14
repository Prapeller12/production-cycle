import sys
from pathlib import Path
import pefile

path = Path(sys.argv[1])
pe = pefile.PE(str(path))
imports = sorted(entry.dll.decode('ascii').lower() for entry in pe.DIRECTORY_ENTRY_IMPORT)
external_crt = [name for name in imports if name.startswith(('vcruntime', 'msvcp', 'concrt'))]
assert not external_crt, f'Non-system Visual C++ Runtime dependency: {external_crt}'
assert pe.FILE_HEADER.Machine == 0x8664, 'Expected Windows x64'
assert pe.OPTIONAL_HEADER.Subsystem == 2, 'Double-click launcher must use the Windows GUI subsystem'
print({'file': path.name, 'windows_x64': True, 'external_visual_cpp_runtime': external_crt, 'imports': imports})
