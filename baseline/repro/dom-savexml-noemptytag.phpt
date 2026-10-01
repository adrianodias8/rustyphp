--TEST--
DOMDocument::saveXML() with LIBXML_NOEMPTYTAG
--FILE--
<?php
$tag = "<a href=\"/\" class=\"x\" data-drupal-link-system-path=\"&lt;front&gt;\">";
$dom = new DOMDocument();
@$dom->loadHTML("<!DOCTYPE html><html><body>" . $tag . "</body></html>");
$node = $dom->getElementsByTagName("body")->item(0)->firstChild;
$node->setAttribute("class", $node->getAttribute("class") . " is-active");
$node->setAttribute("aria-current", "page");
var_dump($dom->saveXML($node, LIBXML_NOEMPTYTAG));
var_dump($dom->saveXML($node));
$d2 = new DOMDocument(); $d2->loadXML("<r><e/><f>x</f></r>");
var_dump($d2->saveXML($d2->documentElement, LIBXML_NOEMPTYTAG), $d2->saveXML(null, LIBXML_NOEMPTYTAG));
?>
--EXPECT--
string(101) "<a href="/" class="x is-active" data-drupal-link-system-path="&lt;front&gt;" aria-current="page"></a>"
string(98) "<a href="/" class="x is-active" data-drupal-link-system-path="&lt;front&gt;" aria-current="page"/>"
string(22) "<r><e></e><f>x</f></r>"
string(45) "<?xml version="1.0"?>
<r><e></e><f>x</f></r>
"
