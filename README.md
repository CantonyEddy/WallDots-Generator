# WallDots

Transforme une image en grille de points type **halftone** : la taille de
chaque point suit la luminosité locale, et sa couleur reprend la teinte
**dominante** des pixels environnants. Idéal pour se fabriquer des fonds
d'écran. Une option permet de passer l'image en noir & blanc avant la
transformation.

Écrit en Rust — un binaire unique, aucune dépendance à l'exécution. Utilisable
en ligne de commande (scriptable), via une interface interactive avec aperçu
en temps réel (`--tui`), ou via un explorateur de fichiers intégré (`walldots`
sans argument).

## Installation

### Depuis les sources

```sh
git clone https://github.com/CantonyEddy/WallDots-Generator.git
cd WallDots-Generator
cargo build --release
# le binaire est dans ./target/release/walldots
install -Dm755 target/release/walldots ~/.local/bin/walldots
```

### Arch Linux (AUR)

```sh
# une fois le paquet publié :
yay -S walldots
```

Un `PKGBUILD` est fourni à la racine du dépôt pour construire le paquet
localement :

```sh
makepkg -si
```

## Utilisation

```sh
walldots image.jpg
```

Génère par défaut `image_dots.png` et `image_dots.svg` à côté de l'image
source.

### Options

| Option | Défaut | Description |
|---|---|---|
| `-o, --output <base>` | `<image>_dots` | Base du chemin de sortie (les extensions sont ajoutées). |
| `-g, --grid <n>` | `100` | Nombre de points sur la largeur (résolution de la grille). |
| `--scale <f>` | `1.0` | Facteur d'échelle de l'image de sortie (`2.0` = deux fois plus grand). |
| `--bw <mode>` | `none` | Conversion N&B : `none`, `grayscale`, `threshold`. |
| `--threshold <f>` | `0.5` | Seuil de binarisation pour `--bw threshold` (0.0–1.0). |
| `--min-radius <f>` | `0.0` | Rayon minimal du point, en fraction du demi-pas de la grille. |
| `--max-radius <f>` | `1.0` | Rayon maximal du point, en fraction du demi-pas (`>1.0` = les points se chevauchent). |
| `--gamma <f>` | `1.0` | Courbe appliquée à la luminosité : `>1` assombrit, `<1` éclaircit. |
| `--invert` | — | Inverse la relation luminosité→taille (par défaut : clair = gros point). |
| `--bg <hex>` | `#000000` | Couleur de fond (`#rrggbb`, `rrggbb`, `#rgb`). |
| `--shape <s>` | `circle` | Forme des points : `circle`, `square` (bords arrondis), `triangle`, `hexagon`. |
| `-f, --format <fmt>` | `both` | Format(s) de sortie : `png`, `svg`, `both`. |
| `-t, --tui` | — | Ouvre l'interface interactive avec aperçu en temps réel. |

L'aide de `walldots --help` est regroupée par fonctionnalité (Transformation en
points, Noir & blanc, Sortie & mode).

## Explorateur de fichiers

Lancé **sans argument**, `walldots` ouvre un explorateur de fichiers intégré
(façon `yazi`) pour parcourir tes dossiers, prévisualiser les images et en
choisir une :

```sh
walldots
```

À gauche la liste du dossier courant (dossiers puis images, `../` pour
remonter) ; à droite un aperçu **en vraie image** de l'image survolée (voir la
note sur la qualité plus bas). `↑`/`↓` pour naviguer,
`↵`/`→` pour entrer dans un dossier ou choisir une image (elle s'ouvre alors
dans le mode interactif), `←` pour remonter, `q` pour quitter. Depuis le mode
interactif, la touche `o` rouvre l'explorateur pour changer d'image.

Une **barre de recherche** (au-dessus de l'arborescence) filtre les grandes
bibliothèques : appuie sur `/`, tape pour filtrer les noms en direct, `↑`/`↓`
pour parcourir les résultats, `↵` pour choisir, `Échap` pour effacer le filtre.

## Mode interactif (TUI)

```sh
walldots image.jpg --tui
```

Ouvre une interface terminal avec, à gauche, tous les paramètres réglables et,
à droite, un aperçu du rendu qui se met à jour à chaque modification (voir la
note « Qualité de l'aperçu » plus bas).

Raccourcis :

| Touche | Action |
|---|---|
| `↑` / `↓` (ou `k` / `j`) | Naviguer entre les paramètres |
| `←` / `→` (ou `h` / `l`) | Ajuster la valeur pas à pas |
| `entrée` | Saisir une valeur au clavier (grille, échelle, rayons, gamma, fond hex) ; cycler pour les champs à choix. `entrée` valide, `Échap` annule |
| `espace` | Cycler (mode N&B, forme, fond, inversion, format) |
| `z` | Zoom de l'aperçu (recadrage central agrandi) pour voir la forme des points |
| `o` | Ouvrir l'explorateur de fichiers pour changer d'image |
| `s` | Sauvegarder (PNG/SVG selon la config) |
| `q` / `Échap` | Quitter |

Tous les paramètres de la CLI sont réglables dans la TUI, y compris le format de sortie (`png`, `svg`, ou les deux).

L'aperçu travaille sur une version réduite de l'image pour rester fluide ; la
sauvegarde (`s`) génère toujours le rendu à pleine résolution.

La touche `z` bascule sur un aperçu zoomé (recadrage central agrandi) pour
inspecter la forme des points de près.

### Qualité de l'aperçu

Les aperçus (explorateur et tuner) utilisent [`ratatui-image`](https://crates.io/crates/ratatui-image) :
sur un terminal qui supporte un protocole graphique (**Kitty**, ex. kitty,
ghostty, wezterm), l'image s'affiche en pleine qualité. Sur les autres
terminaux, un repli automatique en **demi-blocs Unicode** garantit que ça
fonctionne partout (à résolution moindre). Aucune bibliothèque C n'est requise.

L'interface n'utilise que les **couleurs ANSI du terminal** (pas de couleur
codée en dur) : elle suit donc ta palette et s'adapte automatiquement aux
thèmes dynamiques (pywal, wallust, …).

### Exemples

Rendu couleur fin, uniquement en PNG :

```sh
walldots photo.jpg -g 160 -f png
```

Halftone noir & blanc classique sur fond noir :

```sh
walldots portrait.jpg --bw grayscale -g 120 --bg "#000000"
```

Binarisation dure (deux niveaux) avec des points carrés :

```sh
walldots logo.png --bw threshold --threshold 0.55 --shape square
```

Wallpaper haute résolution (grille dense, sortie agrandie) :

```sh
walldots paysage.jpg -g 200 --scale 2 --gamma 0.8 -f png
```

## Comment ça marche

1. **Chargement** de l'image (PNG, JPEG, WebP, …).
2. **Prétraitement** optionnel en niveaux de gris ou binarisation.
3. **Découpage** en une grille de cellules (résolution réglée par `--grid`).
4. Pour chaque cellule : la **luminosité moyenne** fixe le rayon du point, et
   la **couleur dominante** (mode de l'histogramme quantifié) fixe sa couleur.
5. **Rendu** en SVG (vectoriel, net à toute taille) et/ou PNG anti-crénelé.

## Licence

[GPL-3.0-or-later](LICENSE).
