import struct, sys, os

def read_imports(path):
    data = open(path, 'rb').read()
    e_lfanew = struct.unpack_from('<I', data, 0x3C)[0]
    coff = e_lfanew + 4
    nsec = struct.unpack_from('<H', data, coff + 2)[0]
    optsz = struct.unpack_from('<H', data, coff + 16)[0]
    opt = coff + 20
    magic = struct.unpack_from('<H', data, opt)[0]
    pe32p = magic == 0x20B
    dd = opt + (112 if pe32p else 96)
    imp_rva, imp_sz = struct.unpack_from('<II', data, dd + 8)
    secs = []
    so = opt + optsz
    for i in range(nsec):
        s = so + i * 40
        va = struct.unpack_from('<I', data, s + 12)[0]
        vsz = struct.unpack_from('<I', data, s + 8)[0]
        raw = struct.unpack_from('<I', data, s + 20)[0]
        rsz = struct.unpack_from('<I', data, s + 16)[0]
        secs.append((va, max(vsz, rsz), raw))
    def rva2off(rva):
        for va, vsz, raw in secs:
            if va <= rva < va + vsz:
                return raw + (rva - va)
        return None
    dlls = []
    off = rva2off(imp_rva)
    if off is None:
        return dlls
    while True:
        ent = data[off:off + 20]
        if len(ent) < 20:
            break
        name_rva = struct.unpack_from('<I', ent, 12)[0]
        oft = struct.unpack_from('<I', ent, 0)[0]
        if name_rva == 0 and oft == 0:
            break
        no = rva2off(name_rva)
        if no is None:
            break
        end = data.index(b'\x00', no)
        dlls.append(data[no:end].decode('ascii', 'replace'))
        off += 20
    return dlls

exe = sys.argv[1]
dlls = read_imports(exe)
dist = os.path.dirname(exe)
sysdir = os.environ.get('WINDIR', r'C:\Windows') + r'\System32'
for d in dlls:
    local = os.path.exists(os.path.join(dist, d))
    sysok = os.path.exists(os.path.join(sysdir, d))
    status = 'OK' if (local or sysok) else 'MISSING'
    print(f'{status:8} {d}  (dist={local}, system={sysok})')
