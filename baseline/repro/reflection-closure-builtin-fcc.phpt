--TEST--
ReflectionFunction on first-class callables and closures: name, closure this/called/scope class (a builtin's FCC included)
--FILE--
<?php
class A {
    public function m() {}
    public static function sm() {}
    private function pm() {}
    public function closures(): array {
        return [
            'method fcc' => $this->m(...),
            'private fcc' => $this->pm(...),
            'static fcc' => self::sm(...),
            'fromCallable method' => Closure::fromCallable([$this, 'm']),
            'fromCallable static str' => Closure::fromCallable('A::sm'),
            'closure in method' => function () {},
            'static closure in method' => static function () {},
            'arrow in method' => fn () => 1,
        ];
    }
}
function f() {}
$list = (new A)->closures() + [
    'function fcc' => f(...),
    'builtin fcc' => strlen(...),
    'fromCallable fn' => Closure::fromCallable('f'),
    'top closure' => function () {},
    'fromCallable closure' => Closure::fromCallable(function () {}),
];
foreach ($list as $k => $c) {
    $r = new ReflectionFunction($c);
    $this_ = $r->getClosureThis();
    $called = $r->getClosureCalledClass();
    $scope = $r->getClosureScopeClass();
    printf("%-26s name=%s getName=%s this=%s called=%s scope=%s\n", $k,
        preg_replace('/:\d+\}/', ':N}', $r->name), preg_replace('/:\d+\}/', ':N}', $r->getName()),
        $this_ ? get_class($this_) : '-', $called ? $called->name : '-', $scope ? $scope->name : '-');
}
$r = new ReflectionFunction(strtoupper(...));
var_dump($r->name, $r->isInternal(), $r->isClosure(), $r->getClosureThis(), $r->getClosure()('ok'));
?>
--EXPECTF--
method fcc                 name=m getName=m this=A called=A scope=A
private fcc                name=pm getName=pm this=A called=A scope=A
static fcc                 name=sm getName=sm this=- called=A scope=A
fromCallable method        name=m getName=m this=A called=A scope=A
fromCallable static str    name=sm getName=sm this=- called=A scope=A
closure in method          name={closure:A::closures():N} getName={closure:A::closures():N} this=A called=A scope=A
static closure in method   name={closure:A::closures():N} getName={closure:A::closures():N} this=- called=A scope=A
arrow in method            name={closure:A::closures():N} getName={closure:A::closures():N} this=A called=A scope=A
function fcc               name=f getName=f this=- called=- scope=-
builtin fcc                name=strlen getName=strlen this=- called=- scope=-
fromCallable fn            name=f getName=f this=- called=- scope=-
top closure                name={closure:%s:N} getName={closure:%s:N} this=- called=- scope=-
fromCallable closure       name={closure:%s:N} getName={closure:%s:N} this=- called=- scope=-
string(10) "strtoupper"
bool(true)
bool(true)
NULL
string(2) "OK"
