--TEST--
DOMDocument::saveXML() writes namespace declarations before attributes (libxml2 nsDef order)
--FILE--
<?php
$d = new DOMDocument(); $d->loadXML("<rss version=\"2.0\" xml:base=\"http://x/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><c a=\"1\" xmlns=\"urn:d\" b=\"2\"/></rss>");
echo $d->saveXML();
?>
--EXPECT--
<?xml version="1.0"?>
<rss xmlns:dc="http://purl.org/dc/elements/1.1/" version="2.0" xml:base="http://x/"><c xmlns="urn:d" a="1" b="2"/></rss>
