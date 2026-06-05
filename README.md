# NanoLog

Idée du défi:

```txt
nc -> Rust heapnote binary
  +- [1] Identifier UAF via cve-rs lifetime trick (source distribuée, zéro unsafe)
  +- [2] Heap leak + libc leak via UAF read sur chunk libéré
  +- [3] Bypass safe-linking glibc 2.35 (XOR heap >> 12)
  +- [4] Tcache poisoning -> alloc arbitraire -> `__environ` leak -> stack pivot + ROP
  +- [5] Shell dans le container
       +- cat /proc/1/cmdline -> "systemd" -> `pid=host` détecté
       +- cat /proc/1/root/home/ctf/.env -> FLAG
```
