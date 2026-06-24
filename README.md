# NanoLog, A.K.A. "Unsound Memories" & "Deeper Memories"

> **DCI Summer Camp 2026** -- catégorie **PWN** -- auteur [Vianpyro](https://github.com/Vianpyro)

Un challenge d'exploitation de heap écrit en **Rust**, **sans un seul `unsafe`**, qui
exploite un véritable trou de soundness du compilateur Rust (coercion de durées
de vie via variance) pour fabriquer un [Use-After-Free](https://en.wikipedia.org/wiki/Dangling_pointer). 
Deux flags, une seule primitive.

## 🇫🇷 Le défi

NanoLog est un petit gestionnaire d'enregistrements en ligne de commande
(logs, références, administrateurs) exposé via `socat` sur le port 1337. Le code
source complet est distribué : c'est volontaire. Tout l'intérêt du challenge est
qu'aucun mot-clé `unsafe` n'apparaît, alors que le programme contient malgré tout
un comportement indéfini bien réel.

### Flag 1 -- `Admin Bypass` (`CFSS:0.3/TS:I/E:L/HSFC:Y=5-9`=**7**)

La fonction `cache_ref` transforme une référence de durée de vie courte en une
référence `'static` en abusant de la variance des lifetimes (le bug rustc
[#25860](https://github.com/rust-lang/rust/issues/25860)). `alloc_ref` s'en sert
pour retourner une référence vers un `Box` détruit en fin de fonction : une
référence suspendue (*dangling*).

Comme un `ref` (144 octets) et un `AdminRecord` (144 octets) ont exactement la
même taille, l'allocateur recycle le bloc libéré. La référence suspendue se
retrouve à aliaser un `AdminRecord`. Il suffit alors d'écrire `is_admin = 1` à
l'offset 0 via `ref_edit` pour débloquer le flag.

### Flag 2 -- `Option<fn> niche hijack` (`CFSS:0.3/TS:A/E:H/HSFC:Y=13-20`=**13**)

Même primitive, cible plus profonde : le champ `callback: Option<fn(*const u8)>`
à l'offset 8. Un pointeur de fonction étant garanti non-nul en Rust, le
compilateur encode `None` avec la valeur zéro (*niche optimization*) -- pas
d'octet de discriminant. Écrire 8 octets non-nuls à l'offset 8 **fabrique un
`Some(f)` que le code Rust n'a jamais construit**.

Le binaire est [PIE](https://en.wikipedia.org/wiki/Position-independent_code),
donc il faut d'abord fuiter la base : le callback par défaut est `Some(banner)`,
et lire l'offset 8 via la vue `ref` donne l'adresse runtime de `banner`.
On en déduit la base, on calcule l'adresse de la fonction cachée
`win` (qui lit `/flag2`), on forge `Some(win)` à l'offset 8, et `admin_show`
exécute le tout.

## 🇬🇧 The challenge

NanoLog is a small command-line record manager (logs, refs, admins) exposed over
`socat` on port 1337. The full source is shipped on purpose: the whole point is
that the program contains **no `unsafe` keyword** yet still exhibits genuine
undefined behaviour.

**Flag 1 -- Admin Bypass.** `cache_ref` launders a short-lived reference into a
`'static` one by abusing lifetime variance (rustc soundness hole
[#25860](https://github.com/rust-lang/rust/issues/25860)). `alloc_ref` returns a
reference to a `Box` dropped at function exit -- a dangling pointer. A `ref` and
an `AdminRecord` are both 144 bytes, so the allocator recycles the freed chunk
and the dangling reference ends up aliasing an `AdminRecord`. Writing
`is_admin = 1` at offset 0 via `ref_edit` unlocks the flag.

**Flag 2 -- `Option<fn>` niche hijack.** Same primitive, deeper target: the
`callback: Option<fn(*const u8)>` field at offset 8. Function pointers are
non-null in Rust, so the compiler encodes `None` as zero (niche optimization).
Writing 8 non-zero bytes at offset 8 forges a `Some(f)` the Rust code never
built. Defeat PIE by leaking the default `Some(banner)` callback to recover the
base, compute the address of the hidden `win` function (which reads `/flag2`),
forge `Some(win)`, and let `admin_show` call it.

## Structure

```
.
├── src/
│   ├── lib.rs        # logique du challenge (State, AdminRecord, cache_ref, win)
│   └── main.rs       # boucle d'interaction / menu
├── solution/
│   ├── flag1/README.md   # write-up détaillé (FR + EN)
│   └── flag2/README.md   # write-up détaillé (FR + EN)
├── solve.py          # exploit autonome des deux flags (pwntools)
├── Dockerfile        # build + runtime (socat)
├── compose.yml       # déploiement local
└── rust-toolchain.toml
```

> [!WARNING]
> **Contrainte de toolchain.** Le challenge cible **Rust 1.79.0**.
> [Le trou de soundness #25860](https://github.com/rust-lang/rust/issues/25860) est corrigé dans les versions du compilateur récentes ;
> `rust-toolchain.toml` épingle la version pour garantir la reproductibilité.

## Build & run

### Docker (recommandé)

```sh
docker compose up --build
nc localhost 1337
```

Les flags sont injectés au build/runtime (jamais en dur dans le source) :
`FLAG1` via variable d'environnement, `FLAG2` via le fichier `/flag2`.

### Local

```sh
cargo build --release
FLAG1='DCI{...}' ./target/release/nanolog   # nécessite aussi /flag2
```

> Le binaire refuse de démarrer si `FLAG1` ou `/flag2` sont absents.

## Solve

`solve.py` résout les deux flags de bout en bout. Il lit les offsets statiques de
`banner` et `win` depuis un binaire de référence non-strippé (profil
`release-syms`), ce qui le garde robuste aux recompilations.

```sh
cargo build --profile release-syms        # produit nanolog-release-syms
python3 solve.py [HOST] [PORT]
```

## Idée de design

Le challenge récompense la **lecture du code** plutôt que le reversing : la
distinction « zéro `unsafe` mais UB bien réel » n'est visible qu'en lisant la
source. Distribuer le binaire seul aurait masqué tout l'intérêt pédagogique.

Concepts couverts : variance des lifetimes, soundness de Rust, recyclage de
chunks par l'allocateur, `#[repr(C)]`, et la niche optimization de `Option<fn>`.

## Licence

Distribué sous licence [BSD Zero Clause License (0BSD)](LICENSE) ; réutilisation
libre, sans condition d'attribution. Rejouez, adaptez ou réemployez ce challenge
comme bon vous semble.
