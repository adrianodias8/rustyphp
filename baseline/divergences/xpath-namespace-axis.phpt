--TEST--
XPath namespace:: axis returns the in-scope namespace nodes
--FILE--
<?php
$d = new DOMDocument();
$d->loadXML('<r xmlns:a="urn:a"><c xmlns:b="urn:b"/></r>');
$x = new DOMXPath($d);
$c = $d->getElementsByTagName('c')->item(0);
$out = [];
foreach ($x->query('namespace::*', $c) as $ns) { $out[] = $ns->nodeName . '=' . $ns->nodeValue; }
sort($out);
echo implode("\n", $out), "\n";
?>
--EXPECT--
xmlns:a=urn:a
xmlns:b=urn:b
xmlns:xml=http://www.w3.org/XML/1998/namespace
