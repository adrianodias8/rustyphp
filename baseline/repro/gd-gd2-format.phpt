--TEST--
imagegd()/imagegd2() write libgd's GD and GD2 formats byte for byte; imagecreatefromgd()/gd2()/gd2part()/fromstring() read them
--EXTENSIONS--
gd
--FILE--
<?php
var_dump(function_exists('imagegd2'), function_exists('imagegd'));
function out(callable $f): string { ob_start(); $f(); return ob_get_clean(); }

$im = imagecreatetruecolor(5, 4);
imagefilledrectangle($im, 0, 0, 4, 3, imagecolorallocate($im, 10, 20, 30));
imagesetpixel($im, 2, 1, imagecolorallocatealpha($im, 200, 100, 50, 64));
$p = imagecreate(3, 3);
imagecolorallocate($p, 1, 2, 3);
imagesetpixel($p, 1, 1, imagecolorallocate($p, 9, 8, 7));
imagecolortransparent($p, 1);
foreach ([
    'tc gd2 raw' => fn () => imagegd2($im),
    'tc gd2 z64' => fn () => imagegd2($im, null, 64, IMG_GD2_COMPRESSED),
    'tc gd' => fn () => imagegd($im),
    'pal gd2 raw' => fn () => imagegd2($p),
    'pal gd2 z' => fn () => imagegd2($p, null, 0, IMG_GD2_COMPRESSED),
    'pal gd' => fn () => imagegd($p),
] as $k => $f) {
    $b = out($f);
    echo $k, ': ', strlen($b), ' ', md5($b), "\n";
}

// several chunks (cs 64 on 150x70), both formats, read back whole and in part
$big = imagecreatetruecolor(150, 70);
for ($y = 0; $y < 70; $y++) {
    for ($x = 0; $x < 150; $x++) {
        imagesetpixel($big, $x, $y, ($x * 7 + $y * 3) << 8 | ($x ^ $y) | ($y % 5) << 24);
    }
}
$f = tempnam(sys_get_temp_dir(), 'gd2');
foreach ([IMG_GD2_RAW, IMG_GD2_COMPRESSED] as $mode) {
    var_dump(imagegd2($big, $f, 64, $mode));
    echo 'file ', filesize($f), ' ', md5_file($f), "\n";
    $back = imagecreatefromgd2($f);
    $same = true;
    for ($y = 0; $y < 70; $y++) for ($x = 0; $x < 150; $x++) $same = $same && imagecolorat($back, $x, $y) === imagecolorat($big, $x, $y);
    echo imagesx($back), 'x', imagesy($back), ' identical: ', var_export($same, true), "\n";
    $part = imagecreatefromgd2part($f, 60, 50, 10, 30);
    printf("part %dx%d %08x %08x %08x\n", imagesx($part), imagesy($part), imagecolorat($part, 0, 0), imagecolorat($part, 9, 19), imagecolorat($part, 9, 29));
}
var_dump(imagegd($p, $f));
$g = imagecreatefromgd($f);
printf("gd %dx%d tc=%d total=%d transparent=%d %s\n", imagesx($g), imagesy($g), imageistruecolor($g), imagecolorstotal($g), imagecolortransparent($g), json_encode(imagecolorsforindex($g, imagecolorat($g, 1, 1))));
file_put_contents($f, out(fn () => imagegd2($im, null, 64, IMG_GD2_COMPRESSED)));
$s = imagecreatefromstring(file_get_contents($f));
printf("fromstring %dx%d %08x\n", imagesx($s), imagesy($s), imagecolorat($s, 2, 1));
unlink($f);

var_dump(imagegd2($im, '/nonexistent-dir/x.gd2'));
try { imagecreatefromgd2part('x', 0, 0, 0, 1); } catch (ValueError $e) { echo $e->getMessage(), "\n"; }
try { imagecreatefromgd2part('x', 0, 0, 1, 0); } catch (ValueError $e) { echo $e->getMessage(), "\n"; }
$junk = tempnam(sys_get_temp_dir(), 'gdx');
file_put_contents($junk, "\xff\xfe\x00\x02\x00\x02\x01");   // a truecolor .gd header cut short
var_dump(@imagecreatefromgd2($junk), @imagecreatefromgd($junk));
unlink($junk);
?>
--EXPECTF--
bool(true)
bool(true)
tc gd2 raw: 103 06caf1746a254c26e77aac854f4c7a7b
tc gd2 z64: 52 ed07bb03da846fb795d6ac20900a6f0c
tc gd: 91 b75a93dbc500a4279572fb72188e01a4
pal gd2 raw: 1058 4a0f943149a93d543cfcf5d4c5221a24
pal gd2 z: 1072 b289096fe3798bb0873e847d25ed16a1
pal gd: 1046 2c3a4ec2f62714fa82bbd6197334bf57
bool(true)
file 42023 8e08c6563748d6955477e112bf8a09df
150x70 identical: true
part 10x30 00023a0e 0001ac00 00000000
bool(true)
file 26761 c12fafc1e5c2a2387adaa7fa39429baf
150x70 identical: true
part 10x30 00023a0e 0001ac00 00000000
bool(true)
gd 3x3 tc=0 total=2 transparent=1 {"red":9,"green":8,"blue":7,"alpha":127}
fromstring 5x4 00683b27

Warning: imagegd2(): Unable to open "/nonexistent-dir/x.gd2" for writing in %s on line %d
bool(false)
imagecreatefromgd2part(): Argument #4 ($width) must be greater than or equal to 1
imagecreatefromgd2part(): Argument #5 ($height) must be greater than or equal to 1
bool(false)
bool(false)
