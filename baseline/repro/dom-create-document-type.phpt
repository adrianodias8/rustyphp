--TEST--
DOMImplementation::createDocumentType() and createDocument() with a doctype
--FILE--
<?php
$impl = new DOMImplementation();
$dt = $impl->createDocumentType("html");
var_dump(get_class($dt), $dt->name, $dt->publicId, $dt->systemId);
$doc = $impl->createDocument(null, "", $dt);
var_dump($doc->doctype->name);
$doc->appendChild($doc->createElement("html"));
echo $doc->saveHTML();
$dt2 = $impl->createDocumentType("svg", "-//W3C//DTD SVG 1.1//EN", "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd");
$d2 = $impl->createDocument(null, "svg", $dt2);
echo $d2->saveXML();
?>
--EXPECT--
string(15) "DOMDocumentType"
string(4) "html"
string(0) ""
string(0) ""
string(4) "html"
<!DOCTYPE html>
<html></html>
<?xml version="1.0"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg/>
