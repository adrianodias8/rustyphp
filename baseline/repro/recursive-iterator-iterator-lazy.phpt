--TEST--
RecursiveIteratorIterator is lazy: modes, max depth, hooks and the current sub-iterator
--FILE--
<?php
$data = ["a" => 1, "b" => ["c" => 2, "d" => ["e" => 3]], "f" => 4];
foreach ([RecursiveIteratorIterator::LEAVES_ONLY, RecursiveIteratorIterator::SELF_FIRST, RecursiveIteratorIterator::CHILD_FIRST] as $mode) {
    $out = [];
    foreach (new RecursiveIteratorIterator(new RecursiveArrayIterator($data), $mode) as $k => $v) {
        $out[] = $k . (is_array($v) ? "[]" : "=$v");
    }
    echo $mode, ": ", implode(" ", $out), "\n";
}
$it = new RecursiveIteratorIterator(new RecursiveArrayIterator($data), RecursiveIteratorIterator::SELF_FIRST);
$it->setMaxDepth(1);
$out = [];
foreach ($it as $k => $v) { $out[] = $k . "@" . $it->getDepth(); }
echo "max1: ", implode(" ", $out), "\n";
var_dump($it->getMaxDepth());
class Hooked extends RecursiveIteratorIterator {
    function beginIteration(): void { echo "beginIteration\n"; }
    function endIteration(): void { echo "endIteration\n"; }
    function beginChildren(): void { echo "beginChildren@", $this->getDepth(), "\n"; }
    function endChildren(): void { echo "endChildren@", $this->getDepth(), "\n"; }
    function nextElement(): void { echo "nextElement ", $this->key(), "\n"; }
}
foreach (new Hooked(new RecursiveArrayIterator($data)) as $k => $v) { echo "  yield $k\n"; }
// Lazy: a mutation made during iteration is seen by elements not yet reached.
$arr = new RecursiveArrayIterator(["x" => 1, "y" => 2]);
$rii = new RecursiveIteratorIterator($arr);
foreach ($rii as $k => $v) {
    echo "$k=$v ", get_class($rii->getInnerIterator()), "\n";
    if ($k === "x") { $arr["y"] = 20; }
}
?>
--EXPECT--
0: a=1 c=2 e=3 f=4
1: a=1 b[] c=2 d[] e=3 f=4
2: a=1 c=2 e=3 d[] b[] f=4
max1: a@0 b@0 c@1 d@1 f@0
int(1)
beginIteration
nextElement a
  yield a
beginChildren@1
nextElement c
  yield c
beginChildren@2
nextElement e
  yield e
endChildren@2
endChildren@1
nextElement f
  yield f
endIteration
x=1 RecursiveArrayIterator
y=20 RecursiveArrayIterator
