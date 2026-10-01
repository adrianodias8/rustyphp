--TEST--
RecursiveDirectoryIterator honours CURRENT_AS_SELF and KEY_AS_FILENAME
--FILE--
<?php
$root = sys_get_temp_dir() . "/rdi-" . getmypid();
mkdir("$root/a/b", 0777, true);
touch("$root/top.txt"); touch("$root/a/mid.txt"); touch("$root/a/b/deep.txt");
$flags = FilesystemIterator::UNIX_PATHS | FilesystemIterator::SKIP_DOTS
       | FilesystemIterator::FOLLOW_SYMLINKS | FilesystemIterator::CURRENT_AS_SELF;
$it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($root, $flags), RecursiveIteratorIterator::SELF_FIRST);
$seen = [];
foreach ($it as $key => $cur) {
    $seen[] = sprintf("%s|%s|%s|%s", get_class($cur), $cur->getSubPath(), $cur->getSubPathname(), substr($key, strlen($root)));
}
sort($seen);
echo implode("\n", $seen), "\n";
$k = new RecursiveDirectoryIterator($root, FilesystemIterator::KEY_AS_FILENAME | FilesystemIterator::SKIP_DOTS);
$keys = iterator_to_array($k);
ksort($keys);
echo implode(",", array_keys($keys)), "\n";
unlink("$root/a/b/deep.txt"); unlink("$root/a/mid.txt"); unlink("$root/top.txt");
rmdir("$root/a/b"); rmdir("$root/a"); rmdir($root);
?>
--EXPECT--
RecursiveDirectoryIterator|a/b|a/b/deep.txt|/a/b/deep.txt
RecursiveDirectoryIterator|a|a/b|/a/b
RecursiveDirectoryIterator|a|a/mid.txt|/a/mid.txt
RecursiveDirectoryIterator||a|/a
RecursiveDirectoryIterator||top.txt|/top.txt
a,top.txt
