# bench/drupal — Drupal 11 under ferro

Everything runs in the dev image; the codebases live on the scratch volume.

```bash
# 1. codebase (oracle composer), once
docker/run.sh bash -c 'cd /scratch && COMPOSER_HOME=/scratch/.composer composer create-project \
  --no-interaction drupal/recommended-project:^11 drupal && cd drupal && \
  COMPOSER_HOME=/scratch/.composer composer require --no-interaction drush/drush'
# 2. install parity (step 1): drush site:install standard on SQLite, fresh copy per run
docker/run.sh /work/php-rust/bench/drupal/install.sh                 # ferro
ENGINE=php docker/run.sh /work/php-rust/bench/drupal/install.sh      # oracle
# 3. the shared base for page parity: the oracle's install, page_cache off,
#    the private key generated once (it is created lazily and randomly,
#    which would make permissionsHash differ between two servers)
docker/run.sh bash -c 'cd /scratch && rm -rf drupal-base && cp -a drupal-php drupal-base && cd drupal-base && \
  php vendor/drush/drush/drush.php pm:uninstall page_cache -y && \
  php vendor/drush/drush/drush.php php:eval "\Drupal::service(\"private_key\")->get();"'
# 4. front-page parity (step 2): oracle php -S vs ferro -S on copies of the base
docker/run.sh /work/php-rust/bench/drupal/frontpage.sh
```

Results: `MISSING_FOR_DRUPAL.md` (what was missing, install and page parity) and `NOTES.md`.
