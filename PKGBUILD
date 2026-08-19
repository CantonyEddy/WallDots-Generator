# Maintainer: RYU <cantony.eddy@gmail.com>
pkgname=walldots
pkgver=2.2.0
pkgrel=1
pkgdesc="Transforme une image en grille de points halftone (taille selon luminosité, couleur dominante locale)"
arch=('x86_64')
url="https://github.com/RYU/WallDots-generator"
license=('GPL-3.0-or-later')
depends=('gcc-libs')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')  # remplacer par le vrai hash au moment de la publication

prepare() {
    cd "WallDots-generator-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "WallDots-generator-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    cd "WallDots-generator-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --release
}

package() {
    cd "WallDots-generator-$pkgver"
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
