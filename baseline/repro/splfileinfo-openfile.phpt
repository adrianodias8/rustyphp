--TEST--
SplFileInfo::openFile() and the stat accessors
--FILE--
<?php
$f = sys_get_temp_dir() . "/sfi-" . getmypid() . ".info.yml";
file_put_contents($f, "name: X\ntype: module\n");
$info = new SplFileInfo($f);
$file = $info->openFile('r');
var_dump(get_class($file));
while (!$file->eof()) { echo "line: ", rtrim((string) $file->fgets()), "\n"; }
var_dump($info->getType(), $info->isExecutable(), $info->getBasename('.info.yml') === basename($f, '.info.yml'));
var_dump($info->getATime() > 0, $info->getCTime() > 0, $info->getInode() > 0, is_int($info->getOwner()), is_int($info->getGroup()));
var_dump(get_class($info->getFileInfo()), $info->getPathInfo()->getPathname() === dirname($f));
unlink($f);
?>
--EXPECT--
string(13) "SplFileObject"
line: name: X
line: type: module
line: 
string(4) "file"
bool(false)
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)
string(11) "SplFileInfo"
bool(true)
