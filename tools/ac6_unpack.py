"""ac6_unpack.py <game path> [<game path>...] [-o OUT]: extract single files from the AC6 Data*.bdt archives.
Game paths like /menu/02_gotreward.gfx. BHD5 headers are RSA-encrypted with the public keys UXM ships (read from its
exe), file data AES-128-ECB over the listed ranges; DCX (Kraken) files are written as is. Needs pycryptodome."""
import argparse, base64, os, re, struct
from Crypto.Cipher import AES

GAME = r"D:\Program Files\Steam\Library\steamapps\common\ARMORED CORE VI FIRES OF RUBICON\Game"
UXM = r"D:\samee\Workspace\Modding\PC\Elden Ring\UXM Selective Unpack 2.4.2.0\UXM Selective Unpack.exe"

def der_int(b, i):
    assert b[i] == 2; i += 1
    n = b[i]; i += 1
    if n & 0x80: k = n & 0x7F; n = int.from_bytes(b[i:i + k], "big"); i += k
    return int.from_bytes(b[i:i + n], "big"), i + n

def uxm_keys():
    d = open(UXM, "rb").read()
    out = []
    for m in re.finditer(re.escape("-----BEGIN RSA PUBLIC KEY-----".encode("utf-16-le")), d):
        end = d.find("-----END RSA PUBLIC KEY-----".encode("utf-16-le"), m.end())
        body = d[m.end():end].decode("utf-16-le")
        der = base64.b64decode("".join(body.split()))
        i = 2 if der[1] < 0x80 else 2 + (der[1] & 0x7F)  # SEQUENCE header
        n, i = der_int(der, i); e, _ = der_int(der, i)
        out.append((n, e))
    return out

def rsa_decrypt(data, n, e):
    k = (n.bit_length() + 7) // 8
    out = bytearray()
    for o in range(0, len(data) - k + 1, k):
        out += pow(int.from_bytes(data[o:o + k], "big"), e, n).to_bytes(k, "big")[1:]
    return bytes(out)

def path_hash(p):
    h = 0
    for c in p.lower().replace("\\", "/"): h = (h * 0x85 + ord(c)) & 0xFFFFFFFFFFFFFFFF
    return h

def parse_bhd(b):
    assert b[:4] == b"BHD5", b[:4]
    nb, boff = struct.unpack_from("<ii", b, 0x10)
    files = {}
    for k in range(nb):
        cnt, off = struct.unpack_from("<ii", b, boff + 8 * k)
        for j in range(cnt):
            h, padded, unpadded, foff, sha, aes = struct.unpack_from("<QiiqQq", b, off + 0x28 * j)
            files[h] = (padded, unpadded, foff, aes)
    return files

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("paths", nargs="+"); ap.add_argument("-o", default="unpacked")
    a = ap.parse_args()
    keys = uxm_keys()
    archives = []
    for name in ("Data0", "Data1", "Data2", "Data3"):
        raw = open(os.path.join(GAME, name + ".bhd"), "rb").read()
        for n, e in keys:
            if (n.bit_length() + 7) // 8 > len(raw): continue
            dec = rsa_decrypt(raw, n, e)
            if dec[:4] == b"BHD5": archives.append((name, parse_bhd(dec), dec)); break
        else: print("no key for", name)
    for p in a.paths:
        h = path_hash(p)
        for name, files, bhd in archives:
            if h not in files: continue
            padded, unpadded, foff, aes = files[h]
            with open(os.path.join(GAME, name + ".bdt"), "rb") as f:
                f.seek(foff); data = bytearray(f.read(padded))
            if aes:
                # AES key block in the BHD: key[16], range count, ranges {start, end} (-1 = not encrypted)
                key = bhd[aes:aes + 16]; nr = struct.unpack_from("<i", bhd, aes + 16)[0]
                c = AES.new(key, AES.MODE_ECB)
                for r in range(nr):
                    s0, s1 = struct.unpack_from("<qq", bhd, aes + 20 + 16 * r)
                    if s0 >= 0 and s1 > s0: data[s0:s1] = c.decrypt(bytes(data[s0:s1]))
            out = os.path.join(a.o, p.lstrip("/").replace("/", os.sep))
            os.makedirs(os.path.dirname(out), exist_ok=True)
            open(out, "wb").write(data[:unpadded or padded])
            print("%s: %s @%#x size %d -> %s" % (p, name, foff, unpadded or padded, out)); break
        else: print(p, "not found")

if __name__ == "__main__": main()
