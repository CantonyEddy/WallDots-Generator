# WallDots

Transforme une image en grille de points type **halftone** : la taille de
chaque point suit la luminosité locale, et sa couleur reprend la teinte
**dominante** des pixels environnants. Idéal pour se fabriquer des fonds
d'écran. Une option permet de passer l'image en noir & blanc avant la
transformation.

Écrit en Rust — un binaire unique, aucune dépendance à l'exécution. Utilisable
en ligne de commande (scriptable) ou via une interface interactive avec aperçu
en temps réel (`--tui`).

## Installation

### Depuis les sources

```sh
git clone https://github.com/RYU/WallDots-generator.git
cd WallDots-generator
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

## Mode interactif (TUI)

```sh
walldots image.jpg --tui
```

Ouvre une interface terminal avec, à gauche, tous les paramètres réglables et,
à droite, un aperçu du rendu qui se met à jour à chaque modification. L'aperçu
s'affiche en demi-blocs Unicode : il fonctionne sur n'importe quel terminal
truecolor, sans protocole graphique particulier.

Raccourcis :

| Touche | Action |
|---|---|
| `↑` / `↓` (ou `k` / `j`) | Naviguer entre les paramètres |
| `←` / `→` (ou `h` / `l`) | Diminuer / augmenter la valeur |
| `espace` / `entrée` | Cycler (mode N&B, forme, fond, inversion) |
| `z` | Zoom de l'aperçu (recadrage central agrandi) pour voir la forme des points |
| `s` | Sauvegarder (PNG/SVG selon la config) |
| `q` / `Échap` | Quitter |

L'aperçu travaille sur une version réduite de l'image pour rester fluide ; la
sauvegarde (`s`) génère toujours le rendu à pleine résolution.

À forte densité de grille, chaque point ne couvre qu'environ un caractère du
terminal : la composition et les couleurs sont fidèles, mais la forme exacte
d'un point (cercle, carré arrondi, triangle, hexagone) n'est pas discernable à
cette taille. La touche `z` bascule sur un aperçu zoomé (recadrage central
agrandi) où la forme des points devient visible.

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
