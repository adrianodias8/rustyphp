<?php
// Point Purge's HTTP Bundled Purger at ferro-edge (bench/edge/purge-roundtrip.sh):
//   drush php:script purge-config.php -- <hostname> <port> <header> <value>
// creating the purger instance if needed; one BAN per queue chunk, tag type.
use Drupal\purge_purger_http\Entity\HttpPurgerSettings;

[$host, $port, $header, $value] = array_slice($extra, -4);
$purgers = \Drupal::service('purge.purgers');
$enabled = $purgers->getPluginsEnabled();
$ids = array_keys(array_filter($enabled, fn ($plugin) => $plugin === 'httpbundled'));
if (!$ids) {
    $id = $purgers->createId();
    $enabled[$id] = 'httpbundled';
    $purgers->setPluginsEnabled($enabled);
    $ids = [$id];
}
$s = HttpPurgerSettings::load($ids[0]);
$s->name = 'ferro-edge';
$s->invalidationtype = 'tag';
$s->hostname = $host;
$s->port = (int) $port;
$s->path = '/';
$s->request_method = 'BAN';
$s->scheme = 'http';
$s->headers = [['field' => $header, 'value' => $value]];
$s->save();
echo "purger {$ids[0]}: BAN http://$host:$port/ with $header: $value\n";
