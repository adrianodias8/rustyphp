--TEST--
RecursiveCallbackFilterIterator filters at every depth
--FILE--
<?php
$data = ["a" => 1, "b" => ["c" => 2, "skip" => 3, "d" => ["e" => 4, "skip" => 5]], "skip" => 6];
$it = new RecursiveCallbackFilterIterator(
    new RecursiveArrayIterator($data),
    function ($current, $key, $iterator) {
        return $key !== "skip";
    }
);
foreach (new RecursiveIteratorIterator($it, RecursiveIteratorIterator::SELF_FIRST) as $k => $v) {
    echo $k, "=", is_array($v) ? "array" : $v, "\n";
}
$it->rewind(); $it->next();
var_dump($it instanceof RecursiveIterator, get_class($it->getChildren()));
?>
--EXPECT--
a=1
b=array
c=2
d=array
e=4
bool(true)
string(31) "RecursiveCallbackFilterIterator"
