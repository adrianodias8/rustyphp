--TEST--
Pdo\Sqlite::createCollation() with a PHP comparison callable (ORDER BY and =)
--EXTENSIONS--
pdo_sqlite
--FILE--
<?php
$db = Pdo\Sqlite::connect("sqlite::memory:");
var_dump($db->createCollation("NOCASE_UTF8", fn($a, $b) => strcmp(mb_strtolower($a), mb_strtolower($b))));
$db->exec("CREATE TABLE t (n TEXT COLLATE NOCASE_UTF8)");
foreach (["Émile", "zeta", "Alpha", "émile", "beta"] as $v) { $db->prepare("INSERT INTO t VALUES (?)")->execute([$v]); }
echo implode(",", $db->query("SELECT n FROM t ORDER BY n")->fetchAll(PDO::FETCH_COLUMN)), "\n";
var_dump((int) $db->query("SELECT COUNT(*) FROM t WHERE n = 'ÉMILE'")->fetchColumn());
?>
--EXPECT--
bool(true)
Alpha,beta,zeta,Émile,émile
int(2)
