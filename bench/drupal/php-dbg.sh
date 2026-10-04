#!/usr/bin/env bash
# bench/drupal/php-dbg.sh — the oracle rebuilt with -g, for per-function and
# per-line attribution (the official /usr/local/bin/php is stripped).
# Same tarball (/usr/src/php.tar.xz), compiler, CFLAGS and LDFLAGS as the
# docker-library build, plus -g (no codegen change); gd and sodium are static
# here instead of shared. Installs into /scratch/php-dbg (bin/php), once.
#   docker/run.sh bash /work/php-rust/bench/drupal/php-dbg.sh
set -euo pipefail
PREFIX=/scratch/php-dbg
[[ -x $PREFIX/bin/php && -z "${FORCE:-}" ]] && { echo "$PREFIX/bin/php exists (FORCE=1 rebuilds)"; exit 0; }
rm -rf /scratch/php-dbg-src && mkdir -p /scratch/php-dbg-src
tar -xJf /usr/src/php.tar.xz -C /scratch/php-dbg-src --strip-components=1
cd /scratch/php-dbg-src
CFLAGS="$PHP_CFLAGS -g" CPPFLAGS="$PHP_CPPFLAGS" LDFLAGS="$PHP_LDFLAGS" ./configure \
  --prefix=$PREFIX --build=aarch64-linux-gnu --with-config-file-path=$PREFIX/etc \
  --with-mhash --with-pic --enable-mbstring --enable-mysqlnd --with-password-argon2 \
  --with-sodium --with-pdo-sqlite=/usr --with-sqlite3=/usr --with-curl --with-iconv \
  --with-openssl --with-zlib --with-libdir=lib/aarch64-linux-gnu \
  --enable-gd --with-freetype --with-jpeg --with-webp \
  --disable-phpdbg --disable-cgi >/scratch/php-dbg-configure.log
make -j"$(nproc)" >/scratch/php-dbg-make.log 2>&1
make install-cli install-headers >/dev/null 2>&1 || make install-cli >/dev/null
"$PREFIX/bin/php" -v
